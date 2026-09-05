use spacetimedb::{ReducerContext, Table, Timestamp};
use stalwart_mta_hook_types::Request as MtaHookRequest;

use crate::models::account::{account, admin_identities};
use crate::models::category::{message_categories, subscriptions};
use crate::models::mail_message::{mail_message, MailMessage};
use crate::models::mta::*;
use crate::reducers::delivery::upsert_mail_ingress;
use crate::services::mta::envelope_parser::{
    extract_header, extract_subject_from_request, parse_email_addresses,
};

pub fn insert_mail_message(
    ctx: &ReducerContext,
    queue_id: Option<String>,
    sender_account_id: Option<u64>,
    sender_email: String,
    subject: String,
    from_header: String,
    reply_to: Option<String>,
    date_header: Option<String>,
    message_id: Option<String>,
    cc_header: Option<String>,
    headers_raw: String,
    body_raw: String,
    message_size: u64,
) -> u64 {
    ctx.db
        .mail_message()
        .insert(MailMessage {
            id: 0,
            queue_id,
            received_at: ctx.timestamp,
            sender_account_id,
            sender_email,
            subject: subject.chars().take(500).collect(),
            from_header,
            reply_to,
            date_header,
            message_id,
            cc_header,
            headers_raw,
            body_raw,
            message_size,
        })
        .id
}

pub fn handle_data_stage(
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

    let mut valid_categories: Vec<(u64, String)> = Vec::new();

    log::trace!(
        "envelope: {}",
        serde_json::to_string(&request).unwrap_or_default()
    );

    // Try the canonical SMTP recipients first.
    if let Some(envelope) = &request.envelope {
        for recipient in &envelope.to {
            let to_address = recipient.address.as_str();

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
        stage: "data".to_string(),
        action: action.to_string(),
        timestamp,
        queue_id: request.context.queue.as_ref().map(|q| q.id.clone()),
        category_count: valid_categories.len() as u32,
    });

    // Persist the full message for each accepted category delivery
    if !valid_categories.is_empty() {
        if let Some(message) = &request.message {
            let sender_account_id = ctx
                .db
                .account()
                .email()
                .filter(&from_address.to_string())
                .next()
                .map(|a| a.id);

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
                        .any(|s| s.category_id == *cat_id && s.status.is_active());
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

            let from_header = extract_header(&message.headers, "from")
                .unwrap_or_else(|| from_address.to_string());
            let date_header = extract_header(&message.headers, "date");
            let message_id = extract_header(&message.headers, "message-id");
            let reply_to = extract_header(&message.headers, "reply-to");
            let cc_header = extract_header(&message.headers, "cc");

            let all_headers: Vec<(&str, &str)> = message
                .headers
                .iter()
                .chain(message.server_headers.iter())
                .map(|(n, v)| (n.as_str(), v.as_str()))
                .collect();
            let headers_raw = serde_json::to_string(&all_headers).unwrap_or_default();

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

            let mail_message_id = insert_mail_message(
                ctx,
                queue_id.clone(),
                sender_account_id,
                from_address.to_string(),
                subject.clone(),
                from_header.clone(),
                reply_to.clone(),
                date_header.clone(),
                message_id.clone(),
                cc_header.clone(),
                headers_raw.clone(),
                body_raw.clone(),
                message_size,
            );

            for (category_id, category_email) in &valid_categories {
                ctx.db.received_message().insert(ReceivedMessage {
                    id: 0,
                    mail_message_id,
                    category_id: *category_id,
                    category_email: category_email.clone(),
                    received_at: timestamp,
                });

                let ingress_id = upsert_mail_ingress(
                    ctx,
                    mail_message_id,
                    *category_id,
                    category_email.clone(),
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
