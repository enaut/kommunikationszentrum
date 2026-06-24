use spacetimedb::{ReducerContext, Table, Timestamp, ViewContext};
use stalwart_mta_hook_types::{Request as MtaHookRequest, Stage};

use crate::account::{
    account, account__view, admin_identities, admin_identities__view, is_admin_identity,
};
use crate::delivery;
use crate::mailing::{message_categories, subscriptions, subscriptions__view};

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

/// One row per accepted email delivery, linked to its sender and the target mailing list category.
/// Not directly public — exposed to clients through the `visible_messages` view.
#[spacetimedb::table(accessor = received_message)]
pub struct ReceivedMessage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    /// Stalwart queue ID from `context.queue.id`
    pub queue_id: Option<String>,
    /// When this row was inserted (used for range scan in admin view)
    #[index(btree)]
    pub received_at: Timestamp,
    /// FK → Account.id; None when the sender is not a known SoLaWi member
    pub sender_account_id: Option<u64>,
    /// Raw envelope sender address
    pub sender_email: String,
    /// FK → MessageCategory.id (used for per-category lookup in user view)
    #[index(btree)]
    pub category_id: u64,
    /// The mailing-list address this message was delivered to
    pub category_email: String,
    /// Parsed Subject header (capped at 500 chars)
    pub subject: String,
    /// Raw From header value
    pub from_header: String,
    pub date_header: Option<String>,
    pub message_id: Option<String>,
    pub reply_to: Option<String>,
    pub cc_header: Option<String>,
    /// JSON array of [name, value] pairs covering all headers (original + server-added)
    pub headers_raw: String,
    /// Full message body; empty string when the message exceeds 2 MB
    pub body_raw: String,
    pub message_size: u64,
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

pub(crate) fn handle_connect_stage(
    ctx: &ReducerContext,
    request: &MtaHookRequest,
    timestamp: Timestamp,
) {
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

pub(crate) fn handle_ehlo_stage(
    ctx: &ReducerContext,
    request: &MtaHookRequest,
    timestamp: Timestamp,
) {
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

pub(crate) fn handle_mail_stage(
    ctx: &ReducerContext,
    request: &MtaHookRequest,
    timestamp: Timestamp,
) {
    let from_address = request
        .envelope
        .as_ref()
        .map(|env| env.from.address.as_str())
        .unwrap_or("unknown");

    log::trace!("MAIL stage - From: {}", from_address);

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

pub(crate) fn handle_rcpt_stage(
    ctx: &ReducerContext,
    request: &MtaHookRequest,
    timestamp: Timestamp,
) {
    if let Some(envelope) = &request.envelope {
        for recipient in &envelope.to {
            let to_address = recipient.address.as_str();
            log::trace!("RCPT stage - To: {}", to_address);

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

pub(crate) fn handle_data_stage(
    ctx: &ReducerContext,
    request: &MtaHookRequest,
    timestamp: Timestamp,
) {
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

    log::trace!(
        "DATA stage - From: {}, Size: {}, Subject: {}",
        from_address,
        message_size,
        subject
    );

    let mut to_addresses = Vec::new();
    let mut valid_categories: Vec<(u64, String)> = Vec::new();

    log::trace!(
        "envelope: {}",
        serde_json::to_string(&request).unwrap_or_default()
    );

    // Try the canonical SMTP recipients first.
    if let Some(envelope) = &request.envelope {
        for recipient in &envelope.to {
            let to_address = recipient.address.as_str();
            to_addresses.push(to_address.to_string());

            if let Some(category) = ctx
                .db
                .message_categories()
                .email_address()
                .find(&to_address.to_string())
                .filter(|c| c.active)
            {
                valid_categories.push((category.id, category.email_address.clone()));
            }
        }
    }

    // Fallback: some MTAs rewrite the envelope and only preserve the `To` header.
    if valid_categories.is_empty() {
        if let Some(message) = &request.message {
            if let Some(to_header) = extract_header(&message.headers, "to") {
                let header_addrs = parse_email_addresses(&to_header);
                if !header_addrs.is_empty() {
                    to_addresses = header_addrs.clone();

                    for to_address in header_addrs {
                        if let Some(category) = ctx
                            .db
                            .message_categories()
                            .email_address()
                            .find(&to_address)
                            .filter(|c| c.active)
                        {
                            valid_categories.push((category.id, category.email_address.clone()));
                        }
                    }
                }
            }
        }
    }

    let action = if !valid_categories.is_empty() {
        log::info!(
            "Accepting message for {} valid category deliveries",
            valid_categories.len()
        );
        "accept"
    } else {
        log::warn!("No valid category deliveries found, quarantaining message");
        "quarantine"
    };

    ctx.db.mta_message_log().insert(MtaMessageLog {
        id: 0,
        from_address: from_address.to_string(),
        to_addresses: serde_json::to_string(&to_addresses).unwrap_or_default(),
        subject: subject.chars().take(100).collect(),
        message_size,
        stage: "data".to_string(),
        action: action.to_string(),
        timestamp,
        queue_id: request.context.queue.as_ref().map(|q| q.id.clone()),
    });

    // Persist the full message for each accepted category delivery
    if !valid_categories.is_empty() {
        if let Some(message) = &request.message {
            // Look up sender's SoLaWi account by email (None for external senders)
            let sender_account_id = ctx
                .db
                .account()
                .email()
                .filter(&from_address.to_string())
                .next()
                .map(|a| a.id);

            // Filter valid_categories: only allow if sender is an admin OR has an active subscription to that category
            let sender_is_admin = sender_account_id
                .and_then(|id| ctx.db.account().id().find(&id))
                .map_or(false, |acc| {
                    ctx.db
                        .admin_identities()
                        .identity()
                        .find(&acc.identity)
                        .is_some()
                });

            valid_categories.retain(|(cat_id, cat_email)| {
                if sender_is_admin {
                    return true;
                }
                if let Some(acc_id) = sender_account_id {
                    let has_sub = ctx
                        .db
                        .subscriptions()
                        .subscriber_account_id()
                        .filter(&acc_id)
                        .any(|s| s.category_id == *cat_id && s.active);
                    if !has_sub {
                        log::warn!(
                            "Sender {} (acc {}) is NOT subscribed to category {} ({})",
                            from_address,
                            acc_id,
                            cat_id,
                            cat_email
                        );
                    }
                    has_sub
                } else {
                    log::warn!(
                        "External sender {} attempted to post to category {} ({})",
                        from_address,
                        cat_id,
                        cat_email
                    );
                    false
                }
            });

            if valid_categories.is_empty() {
                log::warn!("No authorized categories left after subscription check");
                return;
            }

            // Extract parsed header fields
            let from_header = extract_header(&message.headers, "from")
                .unwrap_or_else(|| from_address.to_string());
            let date_header = extract_header(&message.headers, "date");
            let message_id = extract_header(&message.headers, "message-id");
            let reply_to = extract_header(&message.headers, "reply-to");
            let cc_header = extract_header(&message.headers, "cc");

            // Combine original and server-added headers into a single JSON array
            let all_headers: Vec<(&str, &str)> = message
                .headers
                .iter()
                .chain(message.server_headers.iter())
                .map(|(n, v)| (n.as_str(), v.as_str()))
                .collect();
            let headers_raw = serde_json::to_string(&all_headers).unwrap_or_default();

            // Skip body storage for very large messages to avoid excessive memory use
            const MAX_BODY_SIZE: usize = 2_000_000;
            let body_raw = if message.size > MAX_BODY_SIZE {
                log::warn!(
                    "Message body exceeds 2 MB ({} bytes), storing headers only",
                    message.size
                );
                String::new()
            } else {
                message.contents.clone()
            };

            let queue_id = request.context.queue.as_ref().map(|q| q.id.clone());

            for (category_id, category_email) in &valid_categories {
                ctx.db.received_message().insert(ReceivedMessage {
                    id: 0,
                    queue_id: queue_id.clone(),
                    received_at: timestamp,
                    sender_account_id,
                    sender_email: from_address.to_string(),
                    category_id: *category_id,
                    category_email: category_email.clone(),
                    subject: subject.chars().take(500).collect(),
                    from_header: from_header.clone(),
                    date_header: date_header.clone(),
                    message_id: message_id.clone(),
                    reply_to: reply_to.clone(),
                    cc_header: cc_header.clone(),
                    headers_raw: headers_raw.clone(),
                    body_raw: body_raw.clone(),
                    message_size,
                });

                let ingress_id = delivery::upsert_mail_ingress(
                    ctx,
                    queue_id.clone(),
                    *category_id,
                    category_email.clone(),
                    sender_account_id,
                    from_address.to_string(),
                    subject.chars().take(500).collect(),
                    from_header.clone(),
                    reply_to.clone(),
                    date_header.clone(),
                    message_id.clone(),
                    cc_header.clone(),
                    headers_raw.clone(),
                    body_raw.clone(),
                    message_size,
                );
                log::info!(
                    "Queued ingress {} for category {} ({})",
                    ingress_id,
                    category_id,
                    category_email
                );
            }
        }
    }
}

pub(crate) fn handle_auth_stage(
    ctx: &ReducerContext,
    _request: &MtaHookRequest,
    timestamp: Timestamp,
) {
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

/// Find the first header whose name (case-insensitive) matches `name` and return its trimmed value.
fn extract_header(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(n, _)| n.to_lowercase() == name)
        .map(|(_, v)| v.trim().to_string())
}

pub(crate) fn extract_subject_from_request(request: &MtaHookRequest) -> String {
    request
        .message
        .as_ref()
        .and_then(|m| extract_header(&m.headers, "subject"))
        .unwrap_or_else(|| "No subject".to_string())
}

/// Parse a `To`-style header value into individual email addresses.
/// This is a permissive, heuristic parser that handles common forms like:
/// - "Alice <alice@example.com>, bob@example.com"
/// - "bob@example.com; carol@example.org"
fn parse_email_addresses(header: &str) -> Vec<String> {
    header
        .split(|c| c == ',' || c == ';')
        .filter_map(|part| {
            let s = part.trim();
            if s.is_empty() {
                return None;
            }
            // Prefer angle-bracket form: Name <addr@domain>
            if let Some(start) = s.find('<') {
                if let Some(end) = s.find('>') {
                    let addr = s[start + 1..end].trim();
                    if addr.contains('@') {
                        return Some(addr.to_string());
                    }
                }
            }
            // Otherwise, take the first whitespace-delimited token that contains '@'
            if let Some(tok) = s.split_whitespace().find(|t| t.contains('@')) {
                let addr = tok
                    .trim_matches(|c: char| c == '<' || c == '>' || c == '"' || c == '\'')
                    .trim()
                    .to_string();
                if addr.contains('@') {
                    return Some(addr);
                }
            }
            // Last-resort: if the whole part contains '@', return it cleaned
            if s.contains('@') {
                Some(
                    s.trim_matches(|c: char| c == '<' || c == '>' || c == '"' || c == '\'')
                        .trim()
                        .to_string(),
                )
            } else {
                None
            }
        })
        .collect()
}

#[spacetimedb::view(accessor = visible_messages, public)]
pub fn visible_messages(ctx: &ViewContext) -> Vec<ReceivedMessage> {
    let sender = ctx.sender();
    let is_admin = ctx.db.admin_identities().identity().find(&sender).is_some();
    if is_admin {
        ctx.db
            .received_message()
            .received_at()
            .filter(Timestamp::UNIX_EPOCH..)
            .collect()
    } else {
        match ctx.db.account().identity().find(&sender) {
            Some(acc) => {
                // Collect subscribed category IDs first to release the borrow on ctx.db
                // before the second round of indexed lookups.
                let subscribed_category_ids: Vec<u64> = ctx
                    .db
                    .subscriptions()
                    .subscriber_account_id()
                    .filter(&acc.id)
                    .filter(|s| s.active)
                    .map(|s| s.category_id)
                    .collect();
                subscribed_category_ids
                    .into_iter()
                    .flat_map(|cat_id| {
                        ctx.db
                            .received_message()
                            .category_id()
                            .filter(&cat_id)
                            .collect::<Vec<_>>()
                    })
                    .collect()
            }
            None => vec![],
        }
    }
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
