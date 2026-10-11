use spacetimedb::{ReducerContext, Table, Timestamp};
use stalwart_mta_hook_types::Request as MtaHookRequest;

use crate::models::account::{account, account_emails, admin_identities};
use crate::models::delivery::{SystemMailPending, system_mail_pending};
use crate::models::mail_message::{MailMessage, mail_message};
use crate::models::mta::*;
use crate::models::topic::{SubscriptionPermission, message_topics, subscriptions};
use crate::reducers::delivery::upsert_mail_ingress;
use crate::services::mta::admission::{SenderEligibility, decide_topic_admission};
use crate::services::mta::envelope_parser::{
    extract_envelope_from_address, extract_from_header_address, extract_header,
    extract_subject_from_request,
};
use crate::services::mta::rejection::{
    RejectedTopic, build_rejection_email, is_oversize_message, is_valid_bounce_recipient,
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

pub fn handle_data_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: Timestamp) {
    let header_from_email = extract_from_header_address(request);
    let envelope_from_email = extract_envelope_from_address(request);

    let message_size = request
        .message
        .as_ref()
        .map(|msg| msg.size as u64)
        .unwrap_or(0);
    let subject = extract_subject_from_request(request);
    let oversize = is_oversize_message(message_size);

    log::trace!(
        "DATA stage - Header From: {:?}, Envelope From: {:?}, Size: {}, Subject: {}",
        header_from_email,
        envelope_from_email,
        message_size,
        subject
    );

    let mut target_topics: Vec<(u64, String, String)> = Vec::new();

    log::trace!(
        "envelope: {}",
        serde_json::to_string(&request).unwrap_or_default()
    );

    if let Some(envelope) = &request.envelope {
        for recipient in &envelope.to {
            let to_address = recipient.address.to_lowercase();

            if let Some(topic) = ctx
                .db
                .message_topics()
                .email_address()
                .find(&to_address)
                .filter(|c| c.active)
            {
                if !target_topics.iter().any(|(id, _, _)| *id == topic.id) {
                    target_topics.push((topic.id, topic.email_address.clone(), topic.name.clone()));
                }
            }
        }
    }

    if target_topics.is_empty() {
        return;
    }

    let find_account_emails = |email: &str| -> Vec<crate::models::account::AccountEmail> {
        let email_lower = email.to_lowercase();
        ctx.db
            .account_emails()
            .email()
            .filter(&email_lower)
            .collect()
    };

    let is_verified_active = |ae: &crate::models::account::AccountEmail| -> bool {
        ae.is_verified
            && ctx
                .db
                .account()
                .id()
                .find(&ae.account_id)
                .map_or(false, |acc| acc.is_active)
    };

    let account_has_admin_identity = |account_id: u64| -> bool {
        ctx.db.account().id().find(&account_id).map_or(false, |acc| {
            acc.is_active
                && ctx
                    .db
                    .admin_identities()
                    .identity()
                    .find(&acc.identity)
                    .is_some()
        })
    };

    // Prefer From-header address, then envelope sender. See parent spec: known
    // still relies on From with envelope as fallback.
    let (from_address, matching_emails) = if let Some(header_addr) = &header_from_email {
        let accounts = find_account_emails(header_addr);
        if accounts.iter().any(is_verified_active)
            || accounts
                .iter()
                .any(|ae| account_has_admin_identity(ae.account_id))
        {
            (header_addr.clone(), accounts)
        } else if let Some(env_addr) = &envelope_from_email {
            let env_accounts = find_account_emails(env_addr);
            if !env_accounts.is_empty() {
                (env_addr.clone(), env_accounts)
            } else {
                (header_addr.clone(), accounts)
            }
        } else {
            (header_addr.clone(), accounts)
        }
    } else if let Some(env_addr) = &envelope_from_email {
        let env_accounts = find_account_emails(env_addr);
        (env_addr.clone(), env_accounts)
    } else {
        ("unknown".to_string(), Vec::new())
    };

    let sender_is_admin = matching_emails
        .iter()
        .any(|ae| account_has_admin_identity(ae.account_id));

    let verified_active_emails: Vec<_> = matching_emails
        .iter()
        .filter(|ae| is_verified_active(ae))
        .cloned()
        .collect();

    let sender_account_ids: Vec<u64> = verified_active_emails
        .iter()
        .map(|ae| ae.account_id)
        .collect();

    let mut topic_decisions: Vec<(u64, String, String, MessageDisposition)> = Vec::new();

    for (topic_id, topic_email, topic_name) in target_topics {
        let eligibility = if sender_is_admin {
            SenderEligibility::Admin
        } else if verified_active_emails.is_empty() {
            SenderEligibility::NotRegistered
        } else {
            let mut subscribed_active = false;
            let mut write = false;
            for ae in &verified_active_emails {
                for s in ctx.db.subscriptions().account_email_id().filter(&ae.id) {
                    if s.topic_id == topic_id && s.status.is_active() {
                        subscribed_active = true;
                        if matches!(s.permission, SubscriptionPermission::Write) {
                            write = true;
                            break;
                        }
                    }
                }
                if write {
                    break;
                }
            }
            SenderEligibility::VerifiedActive {
                write,
                subscribed_active,
            }
        };

        let disposition = decide_topic_admission(oversize, eligibility);
        topic_decisions.push((topic_id, topic_email, topic_name, disposition));
    }

    let rejected_topics: Vec<RejectedTopic> = topic_decisions
        .iter()
        .filter_map(|(_, topic_email, topic_name, disposition)| match disposition {
            MessageDisposition::Rejected(reason) => Some(RejectedTopic {
                topic_name: topic_name.clone(),
                topic_email: topic_email.clone(),
                reason: *reason,
            }),
            MessageDisposition::Accepted => None,
        })
        .collect();

    if !rejected_topics.is_empty() && is_valid_bounce_recipient(&from_address) {
        let (rejection_subject, rejection_body) = build_rejection_email(
            &subject,
            request.message.as_ref(),
            &rejected_topics,
            &from_address,
            message_size,
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

    if oversize {
        log::warn!(
            "Storing oversized message from {} without body ({} bytes)",
            from_address,
            message_size
        );
    }

    let (from_header, date_header, message_id, reply_to, cc_header, headers_raw, body_raw) =
        if let Some(message) = &request.message {
            let from_header =
                extract_header(&message.headers, "from").unwrap_or_else(|| from_address.clone());
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
            let body_raw = if oversize {
                String::new()
            } else {
                message.contents.clone()
            };
            (
                from_header,
                date_header,
                message_id,
                reply_to,
                cc_header,
                headers_raw,
                body_raw,
            )
        } else {
            (
                from_address.clone(),
                None,
                None,
                None,
                None,
                String::new(),
                String::new(),
            )
        };

    let queue_id = request.context.queue.as_ref().map(|q| q.id.clone());
    let mail_message_id = insert_mail_message(
        ctx,
        queue_id,
        sender_account_ids.first().copied(),
        from_address.clone(),
        subject,
        from_header,
        reply_to,
        date_header,
        message_id,
        cc_header,
        headers_raw,
        body_raw,
        message_size,
    );

    for (topic_id, topic_email, _, disposition) in &topic_decisions {
        ctx.db.received_message().insert(ReceivedMessage {
            id: 0,
            mail_message_id,
            topic_id: *topic_id,
            topic_email: topic_email.clone(),
            received_at: timestamp,
            disposition: *disposition,
        });

        if matches!(disposition, MessageDisposition::Accepted) {
            let ingress_id =
                upsert_mail_ingress(ctx, mail_message_id, *topic_id, topic_email.clone());
            log::info!(
                "Queued ingress {} for topic {} ({})",
                ingress_id,
                topic_id,
                topic_email
            );
        }
    }
}
