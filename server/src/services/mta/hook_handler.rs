use spacetimedb::{ReducerContext, Table, Timestamp};
use stalwart_mta_hook_types::Request as MtaHookRequest;

use crate::models::account::{account, account_emails, admin_identities};
use crate::models::category::{message_categories, subscriptions};
use crate::models::delivery::{system_mail_pending, SystemMailPending};
use crate::models::mail_message::{mail_message, MailMessage};
use crate::models::mta::*;
use crate::reducers::delivery::upsert_mail_ingress;
use crate::services::mta::envelope_parser::{
    extract_header, extract_subject_from_request, parse_email_addresses,
};
use crate::services::mta::rejection::{
    build_rejection_email, is_valid_bounce_recipient, RejectedTopic, TopicRejectionReason,
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

    let mut target_categories: Vec<(u64, String, String)> = Vec::new();

    log::trace!(
        "envelope: {}",
        serde_json::to_string(&request).unwrap_or_default()
    );

    // Try the canonical SMTP recipients first.
    if let Some(envelope) = &request.envelope {
        for recipient in &envelope.to {
            let to_address = recipient.address.to_lowercase();

            if let Some(category) = ctx
                .db
                .message_categories()
                .email_address()
                .find(&to_address)
                .filter(|c| c.active)
            {
                if !target_categories.iter().any(|(id, _, _)| *id == category.id) {
                    target_categories.push((
                        category.id,
                        category.email_address.clone(),
                        category.name.clone(),
                    ));
                }
            }
        }
    }

    // Fallback: some MTAs rewrite the envelope and only preserve the `To` header.
    if target_categories.is_empty() {
        if let Some(message) = &request.message {
            if let Some(to_header) = extract_header(&message.headers, "to") {
                let header_addrs = parse_email_addresses(&to_header);
                if !header_addrs.is_empty() {
                    for to_address in header_addrs {
                        let to_address_lower = to_address.to_lowercase();
                        if let Some(category) = ctx
                            .db
                            .message_categories()
                            .email_address()
                            .find(&to_address_lower)
                            .filter(|c| c.active)
                        {
                            if !target_categories.iter().any(|(id, _, _)| *id == category.id) {
                                target_categories.push((
                                    category.id,
                                    category.email_address.clone(),
                                    category.name.clone(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    let from_lower = from_address.to_lowercase();
    let sender_account_ids: Vec<u64> = ctx
        .db
        .account_emails()
        .email()
        .filter(&from_lower)
        .filter(|ae| ae.is_verified)
        .map(|ae| ae.account_id)
        .filter(|acc_id| {
            ctx.db.account().id().find(acc_id).map_or(false, |acc| acc.is_active)
        })
        .collect();

    let sender_is_admin = sender_account_ids.iter().any(|id| {
        ctx.db.account().id().find(id).map_or(false, |acc| {
            ctx.db
                .admin_identities()
                .identity()
                .find(&acc.identity)
                .is_some()
        })
    });

    let mut authorized_categories: Vec<(u64, String)> = Vec::new();
    let mut rejected_topics: Vec<RejectedTopic> = Vec::new();

    for (cat_id, cat_email, cat_name) in target_categories {
        if sender_is_admin {
            authorized_categories.push((cat_id, cat_email));
            continue;
        }

        if sender_account_ids.is_empty() {
            log::warn!(
                "External/unregistered sender {} attempted to post to category {} ({})",
                from_address,
                cat_id,
                cat_email
            );
            rejected_topics.push(RejectedTopic {
                topic_name: cat_name,
                topic_email: cat_email,
                reason: TopicRejectionReason::NotRegistered,
            });
            continue;
        }

        let mut found_subscription = false;
        let mut has_write = false;

        for acc_id in &sender_account_ids {
            for s in ctx.db.subscriptions().subscriber_account_id().filter(acc_id) {
                if s.category_id == cat_id && s.status.is_active() {
                    found_subscription = true;
                    if matches!(
                        s.permission,
                        crate::models::category::SubscriptionPermission::Write
                    ) {
                        has_write = true;
                        break;
                    }
                }
            }
            if has_write {
                break;
            }
        }

        if has_write {
            authorized_categories.push((cat_id, cat_email));
        } else if found_subscription {
            log::warn!(
                "Sender {} (accounts {:?}) is NOT authorized to write to category {} ({})",
                from_address,
                sender_account_ids,
                cat_id,
                cat_email
            );
            rejected_topics.push(RejectedTopic {
                topic_name: cat_name,
                topic_email: cat_email,
                reason: TopicRejectionReason::NoWritePermission,
            });
        } else {
            log::warn!(
                "Sender {} (accounts {:?}) is not subscribed to category {} ({})",
                from_address,
                sender_account_ids,
                cat_id,
                cat_email
            );
            rejected_topics.push(RejectedTopic {
                topic_name: cat_name,
                topic_email: cat_email,
                reason: TopicRejectionReason::NotSubscribed,
            });
        }
    }

    // If any categories were rejected, queue a rejection response from SMTP_SYSTEM_USER
    if !rejected_topics.is_empty() && is_valid_bounce_recipient(from_address) {
        let (rejection_subject, rejection_body) = build_rejection_email(
            &subject,
            request.message.as_ref(),
            &rejected_topics,
            from_address,
        );

        ctx.db.system_mail_pending().insert(SystemMailPending {
            id: 0,
            recipient: from_address.to_string(),
            subject: rejection_subject,
            body_text: rejection_body,
            instance_id: None,
            claimed_at: None,
        });

        log::info!(
            "Queued rejection notice from SMTP_SYSTEM_USER to {} for {} rejected topic(s)",
            from_address,
            rejected_topics.len()
        );
    }

    let (action, cat_count) = if !authorized_categories.is_empty() {
        log::info!(
            "Accepting message for {} valid category deliveries",
            authorized_categories.len()
        );
        ("accept", authorized_categories.len() as u32)
    } else if !rejected_topics.is_empty() {
        log::warn!(
            "Rejecting message: sender {} unauthorized for {} topic(s)",
            from_address,
            rejected_topics.len()
        );
        ("reject", 0)
    } else {
        log::warn!("No valid category deliveries found, quarantining message");
        ("quarantine", 0)
    };

    ctx.db.mta_message_log().insert(MtaMessageLog {
        id: 0,
        stage: "data".to_string(),
        action: action.to_string(),
        timestamp,
        queue_id: request.context.queue.as_ref().map(|q| q.id.clone()),
        category_count: cat_count,
    });

    // Persist the full message for each accepted category delivery
    if !authorized_categories.is_empty() {
        if let Some(message) = &request.message {
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
                sender_account_ids.first().copied(),
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

            for (category_id, category_email) in &authorized_categories {
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

