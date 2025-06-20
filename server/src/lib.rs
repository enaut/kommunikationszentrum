use serde::{Deserialize, Serialize};
use spacetimedb::{ReducerContext, Table, Timestamp};

#[spacetimedb::table(name = person)]
pub struct Person {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct WebhookPayload {
    pub message: String,
    pub sender: String,
    pub timestamp: i64,
}

#[spacetimedb::table(name = webhook_log)]
pub struct WebhookLog {
    pub id: u64,
    pub payload: String,
    pub processed_at: i64,
}

#[spacetimedb::table(name = mta_connection_log)]
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

#[spacetimedb::table(name = mta_message_log)]
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
    pub timestamp: i64,
    pub queue_id: Option<String>,
}

#[spacetimedb::table(name = blocked_ips)]
pub struct BlockedIp {
    #[primary_key]
    pub ip: String,
    pub reason: String,
    pub blocked_at: i64,
    pub active: bool,
}

#[spacetimedb::table(name = message_categories)]
pub struct MessageCategory {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub name: String,
    pub email_address: String,
    pub description: String,
    pub active: bool,
}

#[spacetimedb::table(name = subscriptions)]
pub struct Subscription {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub subscriber_email: String,
    pub category_id: u64,
    pub subscribed_at: i64,
    pub active: bool,
}

#[spacetimedb::reducer(init)]
pub fn init(_ctx: &ReducerContext) {
    // Called when the module is initially published
}

#[spacetimedb::reducer(client_connected)]
pub fn identity_connected(_ctx: &ReducerContext) {
    // Called everytime a new client connects
}

#[spacetimedb::reducer(client_disconnected)]
pub fn identity_disconnected(_ctx: &ReducerContext) {
    // Called everytime a client disconnects
}

#[spacetimedb::reducer]
pub fn add(ctx: &ReducerContext, name: String) {
    ctx.db.person().insert(Person { name });
}

#[spacetimedb::reducer]
pub fn say_hello(ctx: &ReducerContext) {
    for person in ctx.db.person().iter() {
        log::info!("Hello, {}!", person.name);
    }
    log::info!("Hello, World!");
}

#[spacetimedb::reducer]
pub fn handle_webhook(ctx: &ReducerContext, json_payload: String) {
    // Parse the JSON payload
    match serde_json::from_str::<WebhookPayload>(&json_payload) {
        Ok(payload) => {
            log::info!(
                "Received webhook: {} from {}",
                payload.message,
                payload.sender
            );

            // Store the webhook data
            let log_entry = WebhookLog {
                id: ctx.db.webhook_log().count(),
                payload: json_payload,
                processed_at: payload.timestamp,
            };

            ctx.db.webhook_log().insert(log_entry);

            // Process the webhook data (example: add sender as person)
            if !payload.sender.is_empty() {
                ctx.db.person().insert(Person {
                    name: payload.sender,
                });
            }
        }
        Err(e) => {
            log::error!("Failed to parse webhook payload: {}", e);
        }
    }
}

#[spacetimedb::reducer]
pub fn get_webhook_logs(ctx: &ReducerContext) {
    for log in ctx.db.webhook_log().iter() {
        log::info!("Webhook log {}: {}", log.id, log.payload);
    }
}

#[spacetimedb::reducer]
pub fn handle_mta_hook(ctx: &ReducerContext, hook_data: String) {
    match serde_json::from_str::<serde_json::Value>(&hook_data) {
        Ok(data) => {
            let stage = data["context"]["stage"].as_str().unwrap_or("unknown");
            let timestamp = ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000; // Convert to seconds as i64

            match stage {
                "connect" => handle_connect_stage(ctx, &data, timestamp),
                "ehlo" => handle_ehlo_stage(ctx, &data, timestamp),
                "mail" => handle_mail_stage(ctx, &data, timestamp),
                "rcpt" => handle_rcpt_stage(ctx, &data, timestamp),
                "data" => handle_data_stage(ctx, &data, timestamp),
                "auth" => handle_auth_stage(ctx, &data, timestamp),
                _ => {
                    log::warn!("Unknown MTA hook stage: {}", stage);
                }
            }
        }
        Err(e) => {
            log::error!("Failed to parse MTA hook data: {}", e);
        }
    }
}

fn handle_connect_stage(ctx: &ReducerContext, data: &serde_json::Value, timestamp: u64) {
    let client_ip = data["context"]["client"]["ip"]
        .as_str()
        .unwrap_or("unknown");

    log::info!("Connect stage - IP: [REDACTED]");

    // Check if IP is blocked
    for blocked_ip in ctx.db.blocked_ips().iter() {
        if blocked_ip.ip == client_ip && blocked_ip.active {
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
        client_ip: "[REDACTED]".to_string(),
        stage: "connect".to_string(),
        action: "accept".to_string(),
        timestamp,
        details: "Connection accepted".to_string(),
    });
}

fn handle_ehlo_stage(ctx: &ReducerContext, data: &serde_json::Value, timestamp: u64) {
    let client_ip = "[REDACTED]";
    let helo = data["context"]["client"]["helo"]
        .as_str()
        .unwrap_or("unknown");

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

fn handle_mail_stage(ctx: &ReducerContext, data: &serde_json::Value, timestamp: u64) {
    let from_address = data["envelope"]["from"]["address"]
        .as_str()
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

fn handle_rcpt_stage(ctx: &ReducerContext, data: &serde_json::Value, timestamp: u64) {
    if let Some(to_array) = data["envelope"]["to"].as_array() {
        for recipient in to_array {
            let to_address = recipient["address"].as_str().unwrap_or("unknown");
            log::info!("RCPT stage - To: [REDACTED]");

            // Check if recipient corresponds to a valid category
            let mut category_found = false;
            for category in ctx.db.message_categories().iter() {
                if category.email_address == to_address && category.active {
                    category_found = true;
                    break;
                }
            }

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

fn handle_data_stage(ctx: &ReducerContext, data: &serde_json::Value, timestamp: u64) {
    let from_address = data["envelope"]["from"]["address"]
        .as_str()
        .unwrap_or("unknown");
    let message_size = data["message"]["size"].as_u64().unwrap_or(0);
    let subject = extract_subject_from_headers(data);

    log::info!(
        "DATA stage - From: [REDACTED], Size: {}, Subject: [REDACTED]",
        message_size
    );

    let mut to_addresses = Vec::new();
    let mut valid_categories = Vec::new();

    if let Some(to_array) = data["envelope"]["to"].as_array() {
        for recipient in to_array {
            let to_address = recipient["address"].as_str().unwrap_or("unknown");
            to_addresses.push(to_address.to_string());

            // Find category for this recipient
            for category in ctx.db.message_categories().iter() {
                if category.email_address == to_address && category.active {
                    // Check if sender is subscribed
                    let mut is_subscribed = false;
                    for subscription in ctx.db.subscriptions().iter() {
                        if subscription.subscriber_email == from_address
                            && subscription.category_id == category.id
                            && subscription.active
                        {
                            is_subscribed = true;
                            break;
                        }
                    }

                    if is_subscribed {
                        valid_categories.push(category.id);
                    } else {
                        log::info!("Sender not subscribed to category: [REDACTED]");
                    }
                    break;
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
        subject: if subject.len() > 100 {
            "[REDACTED]".to_string()
        } else {
            subject
        },
        message_size,
        stage: "data".to_string(),
        action: action.to_string(),
        timestamp,
        queue_id: data["context"]["queue"]["id"]
            .as_str()
            .map(|s| s.to_string()),
    });
}

fn handle_auth_stage(ctx: &ReducerContext, _data: &serde_json::Value, timestamp: u64) {
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

fn extract_subject_from_headers(data: &serde_json::Value) -> String {
    if let Some(headers) = data["message"]["headers"].as_array() {
        for header in headers {
            if let Some(header_array) = header.as_array() {
                if header_array.len() >= 2 {
                    let name = header_array[0].as_str().unwrap_or("");
                    let value = header_array[1].as_str().unwrap_or("");
                    if name.to_lowercase() == "subject" {
                        return value.trim().to_string();
                    }
                }
            }
        }
    }
    "No subject".to_string()
}

// Management reducers for categories and subscriptions
#[spacetimedb::reducer]
pub fn add_message_category(
    ctx: &ReducerContext,
    name: String,
    email_address: String,
    description: String,
) {
    ctx.db.message_categories().insert(MessageCategory {
        id: 0,
        name,
        email_address,
        description,
        active: true,
    });
    log::info!("Added new message category");
}

#[spacetimedb::reducer]
pub fn add_subscription(ctx: &ReducerContext, subscriber_email: String, category_id: u64) {
    let timestamp = (ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000) as u64; // Convert to seconds

    ctx.db.subscriptions().insert(Subscription {
        id: 0,
        subscriber_email,
        category_id,
        subscribed_at: timestamp,
        active: true,
    });
    log::info!("Added new subscription");
}

#[spacetimedb::reducer]
pub fn block_ip(ctx: &ReducerContext, ip: String, reason: String) {
    let timestamp = (ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000) as u64; // Convert to seconds

    ctx.db.blocked_ips().insert(BlockedIp {
        ip,
        reason,
        blocked_at: timestamp,
        active: true,
    });
    log::info!("Blocked IP address");
}

#[spacetimedb::reducer]
pub fn get_mta_logs(ctx: &ReducerContext) {
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
