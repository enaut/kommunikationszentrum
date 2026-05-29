use spacetimedb::{ReducerContext, Table, Timestamp};
use stalwart_mta_hook_types::{Request as MtaHookRequest, Stage};

use crate::account::is_admin_identity;
use crate::mailing::{message_categories, subscriptions};

#[spacetimedb::table(accessor = mta_connection_log)]
pub struct MtaConnectionLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub client_ip: String,
    pub stage: String,
    pub action: String,
    pub timestamp: Timestamp,
    pub details: String,
}

#[spacetimedb::table(accessor = mta_message_log)]
pub struct MtaMessageLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub from_address: String,
    pub to_addresses: String, // JSON array as string
    pub subject: String,
    pub message_size: u64,
    pub stage: String,
    pub action: String,
    pub timestamp: Timestamp,
    pub queue_id: Option<String>,
}

#[spacetimedb::table(accessor = blocked_ips)]
pub struct BlockedIp {
    #[primary_key]
    pub ip: String,
    pub reason: String,
    pub blocked_at: Timestamp,
    pub active: bool,
}

#[spacetimedb::reducer]
pub fn handle_mta_hook(ctx: &ReducerContext, hook_data: String) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!(
            "Unauthorized: MTA hook called by non-admin identity {:?}",
            ctx.sender()
        ));
    }

    match serde_json::from_str::<MtaHookRequest>(&hook_data) {
        Ok(request) => {
            let timestamp = ctx.timestamp;

            match request.context.stage {
                Stage::Connect => handle_connect_stage(ctx, &request, timestamp),
                Stage::Ehlo => handle_ehlo_stage(ctx, &request, timestamp),
                Stage::Mail => handle_mail_stage(ctx, &request, timestamp),
                Stage::Rcpt => handle_rcpt_stage(ctx, &request, timestamp),
                Stage::Data => handle_data_stage(ctx, &request, timestamp),
                Stage::Auth => handle_auth_stage(ctx, &request, timestamp),
            }
        }
        Err(e) => {
            log::error!("Failed to parse MTA hook data: {}", e);
        }
    }
    Ok(())
}

fn handle_connect_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: Timestamp) {
    let client_ip = &request.context.client.ip;

    log::info!("Connect stage - IP: [REDACTED]");

    // Check if IP is blocked
    if let Some(blocked) = ctx.db.blocked_ips().ip().find(client_ip) {
        if blocked.active {
            log::warn!("Blocked connection from IP");
            ctx.db.mta_connection_log().insert(MtaConnectionLog {
                id: 0,
                client_ip: "[REDACTED]".to_string(),
                stage: "connect".to_string(),
                action: "reject".to_string(),
                timestamp,
                details: "IP blocked".to_string(),
            });
            return;
        }
    }

    ctx.db.mta_connection_log().insert(MtaConnectionLog {
        id: 0,
        client_ip: client_ip.to_string(),
        stage: "connect".to_string(),
        action: "accept".to_string(),
        timestamp,
        details: "Connection accepted".to_string(),
    });
}

fn handle_ehlo_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: Timestamp) {
    let client_ip = request.context.client.ip.as_str();
    let helo = request.context.client.helo.as_deref().unwrap_or("unknown");

    log::info!("EHLO stage - HELO: [REDACTED]");

    // Basic EHLO validation
    let is_valid = !helo.is_empty() && helo != "unknown";
    let action = if is_valid { "accept" } else { "reject" };
    let details = if is_valid {
        "Valid EHLO/HELO".to_string()
    } else {
        "Invalid EHLO/HELO".to_string()
    };

    ctx.db.mta_connection_log().insert(MtaConnectionLog {
        id: 0,
        client_ip: client_ip.to_string(),
        stage: "ehlo".to_string(),
        action: action.to_string(),
        timestamp,
        details,
    });
}

fn handle_mail_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: Timestamp) {
    let from_address = request
        .envelope
        .as_ref()
        .map(|env| env.from.address.as_str())
        .unwrap_or("unknown");

    log::info!("MAIL stage - From: [REDACTED]");

    // Basic sender validation
    let is_valid = from_address.contains('@') && !from_address.is_empty();
    let action = if is_valid { "accept" } else { "reject" };
    let details = format!(
        "Sender validation: {}",
        if is_valid { "passed" } else { "failed" }
    );

    ctx.db.mta_connection_log().insert(MtaConnectionLog {
        id: 0,
        client_ip: "[REDACTED]".to_string(),
        stage: "mail".to_string(),
        action: action.to_string(),
        timestamp,
        details,
    });
}

fn handle_rcpt_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: Timestamp) {
    if let Some(envelope) = &request.envelope {
        for recipient in &envelope.to {
            let to_address = recipient.address.as_str();
            log::info!("RCPT stage - To: [REDACTED]");

            // O(1) unique-index lookup instead of full table scan
            let category_found = ctx
                .db
                .message_categories()
                .email_address()
                .find(&to_address.to_string())
                .map_or(false, |c| c.active);

            let action = if category_found { "accept" } else { "reject" };
            let details = format!(
                "Category validation: {}",
                if category_found { "found" } else { "not found" }
            );

            ctx.db.mta_connection_log().insert(MtaConnectionLog {
                id: 0,
                client_ip: "[REDACTED]".to_string(),
                stage: "rcpt".to_string(),
                action: action.to_string(),
                timestamp,
                details,
            });
        }
    }
}

fn handle_data_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: Timestamp) {
    let from_address = request
        .envelope
        .as_ref()
        .map(|env| env.from.address.as_str())
        .unwrap_or("unknown");
    let message_size = request
        .message
        .as_ref()
        .map(|msg| msg.size as u64)
        .unwrap_or(0);
    let subject = extract_subject_from_request(request);

    log::info!(
        "DATA stage - From: [REDACTED], Size: {}, Subject: [REDACTED]",
        message_size
    );

    let mut to_addresses = Vec::new();
    let mut valid_categories = Vec::new();

    if let Some(envelope) = &request.envelope {
        for recipient in &envelope.to {
            let to_address = recipient.address.as_str();
            to_addresses.push(to_address.to_string());

            // O(1) unique-index lookup for the category
            if let Some(category) = ctx
                .db
                .message_categories()
                .email_address()
                .find(&to_address.to_string())
                .filter(|c| c.active)
            {
                // O(sender subscriptions) scan via btree index instead of full table scan
                let is_subscribed = ctx
                    .db
                    .subscriptions()
                    .subscriber_email()
                    .filter(&from_address.to_string())
                    .any(|s| s.category_id == category.id && s.active);

                if is_subscribed {
                    valid_categories.push(category.id);
                } else {
                    log::info!("Sender not subscribed to category: [REDACTED]");
                }
            }
        }
    }

    let action = if !valid_categories.is_empty() {
        "accept"
    } else {
        "quarantine"
    };

    ctx.db.mta_message_log().insert(MtaMessageLog {
        id: 0,
        from_address: "[REDACTED]".to_string(),
        to_addresses: serde_json::to_string(&to_addresses).unwrap_or_default(),
        subject: subject.chars().take(100).collect(),
        message_size,
        stage: "data".to_string(),
        action: action.to_string(),
        timestamp,
        queue_id: request.context.queue.as_ref().map(|q| q.id.clone()),
    });
}

fn handle_auth_stage(ctx: &ReducerContext, _request: &MtaHookRequest, timestamp: Timestamp) {
    log::info!("AUTH stage - accepting");

    ctx.db.mta_connection_log().insert(MtaConnectionLog {
        id: 0,
        client_ip: "[REDACTED]".to_string(),
        stage: "auth".to_string(),
        action: "accept".to_string(),
        timestamp,
        details: "Authentication stage - accept".to_string(),
    });
}

fn extract_subject_from_request(request: &MtaHookRequest) -> String {
    if let Some(message) = &request.message {
        for (name, value) in &message.headers {
            if name.to_lowercase() == "subject" {
                return value.trim().to_string();
            }
        }
    }
    "No subject".to_string()
}

#[spacetimedb::reducer]
pub fn block_ip(ctx: &ReducerContext, ip: String, reason: String) {
    let timestamp = ctx.timestamp;

    ctx.db.blocked_ips().insert(BlockedIp {
        ip,
        reason,
        blocked_at: timestamp,
        active: true,
    });
    log::info!("Blocked IP address");
}

#[spacetimedb::reducer]
pub fn dump_mta_logs_to_server_logs(ctx: &ReducerContext) {
    log::info!("=== MTA Connection Logs ===");
    for log in ctx.db.mta_connection_log().iter() {
        log::info!(
            "Connection Log {}: {} - {} - {}",
            log.id,
            log.stage,
            log.action,
            log.details
        );
    }

    log::info!("=== MTA Message Logs ===");
    for log in ctx.db.mta_message_log().iter() {
        log::info!(
            "Message Log {}: {} - {} - Size: {}",
            log.id,
            log.stage,
            log.action,
            log.message_size
        );
    }
}
