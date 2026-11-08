mod composition;
mod jmap;

use std::collections::HashSet;

use spacetimedb::{ProcedureContext, Table, TimeDuration, Timestamp};

use crate::models::{
    account::{account, account_emails},
    delivery::{MailIngress, SystemMailPending, mail_ingress, system_mail_pending},
    mail_message::mail_message,
    sender::{
        JmapDeliveryActor, JmapDeliveryState, JmapMailDelivery, JmapMailDeliveryEvent,
        jmap_mail_deliveries, jmap_mail_delivery_events,
    },
    topic::{
        MessageTopic, SubscriptionUnsubscribeToken, message_topics,
        subscription_unsubscribe_tokens, subscriptions,
    },
};

#[spacetimedb::env]
pub struct Env {
    pub JMAP_DISPATCH_ENABLED: Option<String>,
    pub OIDC_ISSUER_URL: Option<String>,
    pub SYSTEM_JMAP_USERNAME: Option<String>,
    pub SYSTEM_JMAP_PASSWORD: Option<String>,
    pub MESSAGE_ID_DOMAIN: Option<String>,
    pub MAIL_UNSUBSCRIBE_BASE_URL: Option<String>,
}

const DISPATCH_BATCH_SIZE: usize = 10;
const CLAIM_LEASE_MICROS: i64 = 15 * 60 * 1_000_000;

#[derive(Clone)]
struct EligibleRecipient {
    subscription_id: u64,
    email: String,
}

/// Scheduled entry point for native mail fan-out and JMAP delivery.
#[spacetimedb::procedure]
pub fn dispatch_jmap_mail(
    ctx: &mut ProcedureContext,
    _scheduled: crate::models::sender::JmapMailDispatchSchedule,
) -> Result<u32, String> {
    log::info!(
        "dispatch_jmap_mail tick: sender={:?}, db_id={:?}, enabled={:?}",
        ctx.sender(),
        ctx.database_identity(),
        ctx.env.JMAP_DISPATCH_ENABLED()
    );
    if ctx.sender() != ctx.database_identity() {
        log::warn!("dispatch_jmap_mail: unauthorized sender {:?}", ctx.sender());
        return Err("This procedure can only be invoked by its SpacetimeDB schedule".to_string());
    }
    if !ctx
        .env
        .JMAP_DISPATCH_ENABLED()
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
    {
        log::info!("dispatch_jmap_mail: dispatch disabled");
        return Ok(0);
    }

    if let Err(e) = enqueue_system_mail(ctx) {
        log::error!("enqueue_system_mail failed: {e}");
        return Err(e);
    }
    if let Err(e) = fan_out_one_ingress(ctx) {
        log::error!("fan_out_one_ingress failed: {e}");
        return Err(e);
    }
    let claimed = claim_due_deliveries(ctx, DISPATCH_BATCH_SIZE)?;
    if !claimed.is_empty() {
        log::info!("dispatch_jmap_mail: claimed {} deliveries", claimed.len());
    }
    let mut processed = 0;
    for (delivery, claim_token) in claimed {
        if let Err(error) = jmap::dispatch_one(ctx, &delivery, &claim_token) {
            log::warn!(
                "JMAP delivery {} failed: {}",
                delivery.delivery_id,
                error.message
            );
            record_failure(ctx, &delivery, &claim_token, error)?;
        }
        processed += 1;
    }
    Ok(processed)
}

fn enqueue_system_mail(ctx: &mut ProcedureContext) -> Result<(), String> {
    let Some(username) = ctx.env.SYSTEM_JMAP_USERNAME() else {
        return Ok(());
    };
    let password = ctx.env.SYSTEM_JMAP_PASSWORD();
    if password.is_none() {
        return Ok(());
    }
    let domain = ctx
        .env
        .MESSAGE_ID_DOMAIN()
        .unwrap_or_else(|| "localhost".to_string());
    let from_email = if username.contains('@') {
        username
    } else {
        format!("no-reply@{domain}")
    };
    let pending: Vec<SystemMailPending> =
        ctx.with_tx(|tx| tx.db.system_mail_pending().iter().collect());
    let now = ctx.timestamp;

    for mail in pending {
        let delivery_id = format!("system:{}", mail.id);
        let id = delivery_id.clone();
        let from = from_email.clone();
        ctx.with_tx(|tx| {
            if tx
                .db
                .jmap_mail_deliveries()
                .delivery_id()
                .find(&id)
                .is_some()
            {
                return;
            }
            tx.db.jmap_mail_deliveries().insert(JmapMailDelivery {
                delivery_id: id.clone(),
                ingress_id: None,
                topic_id: None,
                subscription_id: None,
                system_mail_id: Some(mail.id),
                legacy_delivery_id: None,
                mail_message_id: None,
                recipient_email: mail.recipient.clone(),
                from_email: from.clone(),
                subject: mail.subject.clone(),
                body_text: mail.body_text.clone(),
                state: JmapDeliveryState::Pending,
                next_attempt_at: now,
                attempt_count: 0,
                claim_token: None,
                claim_expires_at: Timestamp::UNIX_EPOCH,
                jmap_email_id: None,
                jmap_submission_id: None,
                last_error: None,
                created_at: now,
                updated_at: now,
            });
            tx.db
                .jmap_mail_delivery_events()
                .insert(JmapMailDeliveryEvent {
                    id: 0,
                    delivery_id: id.clone(),
                    occurred_at: now,
                    event_type: "jmap_queued".to_string(),
                    attempt_no: 0,
                    jmap_email_id: None,
                    jmap_submission_id: None,
                    details: "System mail queued for native JMAP submission".to_string(),
                    actor: JmapDeliveryActor::Scheduled,
                });
        });
    }
    Ok(())
}

fn fan_out_one_ingress(ctx: &mut ProcedureContext) -> Result<(), String> {
    let caller = ctx.sender();
    let now = ctx.timestamp;
    let ingress = ctx.with_tx(|tx| {
        let mut candidates: Vec<MailIngress> = tx
            .db
            .mail_ingress()
            .iter()
            .filter(|row| {
                matches!(
                    row.claim.status,
                    crate::common::delivery_types::DeliveryStatus::Pending
                        | crate::common::delivery_types::DeliveryStatus::RetryScheduled
                ) && row.claim.next_attempt_at <= now
                    && (row.claim.claim_owner.is_none() || row.claim.claim_expires_at <= now)
            })
            .collect();
        candidates
            .sort_by(|left, right| left.claim.next_attempt_at.cmp(&right.claim.next_attempt_at));
        let Some(mut ingress) = candidates.into_iter().next() else {
            return None;
        };
        ingress.claim.status = crate::common::delivery_types::DeliveryStatus::Processing;
        ingress.claim.claim_owner = Some(caller);
        ingress.claim.claim_expires_at = now + TimeDuration::from_micros(CLAIM_LEASE_MICROS);
        ingress.claim.attempt_count = ingress.claim.attempt_count.saturating_add(1);
        ingress.claim.updated_at = now;
        tx.db.mail_ingress().id().update(ingress.clone());
        Some(ingress)
    });
    let Some(ingress) = ingress else {
        log::debug!("fan_out_one_ingress: no claimable ingress rows found");
        return Ok(());
    };
    log::info!("fan_out_one_ingress: claimed ingress {}", ingress.id);

    let source = ctx.with_tx(|tx| {
        let message = tx.db.mail_message().id().find(&ingress.mail_message_id)?;
        let topic = tx.db.message_topics().id().find(&ingress.topic_id)?;
        let recipients = tx
            .db
            .subscriptions()
            .topic_id()
            .filter(&ingress.topic_id)
            .filter_map(|subscription| {
                if !subscription.status.is_active() {
                    return None;
                }
                let email = tx
                    .db
                    .account_emails()
                    .id()
                    .find(&subscription.account_email_id)?;
                if !email.is_verified || email.account_id != subscription.subscriber_account_id {
                    return None;
                }
                let account = tx
                    .db
                    .account()
                    .id()
                    .find(&subscription.subscriber_account_id)?;
                if !account.is_active {
                    return None;
                }
                Some(EligibleRecipient {
                    subscription_id: subscription.id,
                    email: email.email,
                })
            })
            .collect::<Vec<_>>();
        Some((message, topic, recipients))
    });
    let Some((message, topic, recipients)) = source else {
        log::error!(
            "fan_out_one_ingress: missing canonical message or topic for ingress {}",
            ingress.id
        );
        fail_ingress(ctx, &ingress, "Missing canonical message or topic")?;
        return Ok(());
    };
    log::info!(
        "fan_out_one_ingress: ingress {} has {} recipients (topic {})",
        ingress.id,
        recipients.len(),
        topic.name
    );

    if message.message_size > 0 && message.body_raw.is_empty() {
        fail_ingress(
            ctx,
            &ingress,
            "Canonical message body is unavailable (message exceeded storage limit)",
        )?;
        return Ok(());
    }

    let mut seen_emails = HashSet::new();
    for mut recipient in recipients {
        let normalized_email = recipient.email.trim().to_ascii_lowercase();
        if !seen_emails.insert(normalized_email.clone()) {
            continue;
        }
        recipient.email = normalized_email;

        upsert_unsubscribe_token(ctx, &ingress, &recipient)?;
        upsert_delivery(ctx, &ingress, &message, &topic, &recipient)?;
    }

    ctx.with_tx(|tx| {
        let Some(mut current) = tx.db.mail_ingress().id().find(&ingress.id) else {
            return;
        };
        if current.claim.claim_owner != Some(caller) {
            return;
        }
        current.claim.status = crate::common::delivery_types::DeliveryStatus::Completed;
        current.claim.claim_owner = None;
        current.claim.instance_id = None;
        current.claim.claim_expires_at = Timestamp::UNIX_EPOCH;
        current.claim.last_error = None;
        current.claim.updated_at = now;
        current.completed_at = now;
        tx.db.mail_ingress().id().update(current);
    });
    Ok(())
}

fn upsert_unsubscribe_token(
    ctx: &mut ProcedureContext,
    ingress: &MailIngress,
    recipient: &EligibleRecipient,
) -> Result<(), String> {
    let generated = format!("sub-{}-{}", recipient.subscription_id, uuid_token(ctx)?);
    let owner = ctx.sender();
    let now = ctx.timestamp;
    ctx.with_tx(|tx| {
        let current_ingress = tx.db.mail_ingress().id().find(&ingress.id);
        if !current_ingress.is_some_and(|row| row.claim.claim_owner == Some(owner)) {
            return;
        }
        if let Some(mut existing) = tx
            .db
            .subscription_unsubscribe_tokens()
            .subscription_id()
            .find(&recipient.subscription_id)
        {
            if !existing.active {
                existing.active = true;
                existing.revoked_at = Timestamp::UNIX_EPOCH;
                existing.created_at = now;
                tx.db
                    .subscription_unsubscribe_tokens()
                    .token()
                    .update(existing);
            }
        } else {
            tx.db
                .subscription_unsubscribe_tokens()
                .insert(SubscriptionUnsubscribeToken {
                    token: generated.clone(),
                    subscription_id: recipient.subscription_id,
                    created_at: now,
                    active: true,
                    revoked_at: Timestamp::UNIX_EPOCH,
                });
        }
    });
    Ok(())
}

fn upsert_delivery(
    ctx: &mut ProcedureContext,
    ingress: &MailIngress,
    message: &crate::models::mail_message::MailMessage,
    topic: &MessageTopic,
    recipient: &EligibleRecipient,
) -> Result<(), String> {
    let recipient_hash = blake3::hash(recipient.email.as_bytes()).to_hex();
    let delivery_id = format!(
        "{}:{}:{}",
        ingress.id, recipient.subscription_id, recipient_hash
    );
    let caller = ctx.sender();
    let now = ctx.timestamp;
    let id = delivery_id.clone();
    ctx.with_tx(|tx| {
        let Some(current) = tx.db.mail_ingress().id().find(&ingress.id) else {
            return;
        };
        if current.claim.claim_owner != Some(caller) {
            return;
        }
        if tx
            .db
            .jmap_mail_deliveries()
            .delivery_id()
            .find(&id)
            .is_some()
        {
            return;
        }
        tx.db.jmap_mail_deliveries().insert(JmapMailDelivery {
            delivery_id: id.clone(),
            ingress_id: Some(ingress.id.clone()),
            topic_id: Some(topic.id),
            subscription_id: Some(recipient.subscription_id),
            system_mail_id: None,
            legacy_delivery_id: None,
            mail_message_id: Some(message.id),
            recipient_email: recipient.email.clone(),
            from_email: topic.email_address.clone(),
            subject: message.subject.clone(),
            body_text: String::new(),
            state: JmapDeliveryState::Pending,
            next_attempt_at: now,
            attempt_count: 0,
            claim_token: None,
            claim_expires_at: Timestamp::UNIX_EPOCH,
            jmap_email_id: None,
            jmap_submission_id: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        });
        tx.db
            .jmap_mail_delivery_events()
            .insert(JmapMailDeliveryEvent {
                id: 0,
                delivery_id: id.clone(),
                occurred_at: now,
                event_type: "jmap_queued".to_string(),
                attempt_no: 0,
                jmap_email_id: None,
                jmap_submission_id: None,
                details: "Delivery queued for native JMAP submission".to_string(),
                actor: JmapDeliveryActor::Scheduled,
            });
    });
    Ok(())
}

fn claim_due_deliveries(
    ctx: &mut ProcedureContext,
    limit: usize,
) -> Result<Vec<(JmapMailDelivery, String)>, String> {
    let now = ctx.timestamp;
    let batch_token = uuid_token(ctx)?;
    Ok(ctx.with_tx(|tx| {
        let mut candidates: Vec<JmapMailDelivery> = tx
            .db
            .jmap_mail_deliveries()
            .iter()
            .filter(|row| {
                matches!(
                    row.state,
                    JmapDeliveryState::Pending
                        | JmapDeliveryState::Imported
                        | JmapDeliveryState::SubmissionUnknown
                        | JmapDeliveryState::Importing
                        | JmapDeliveryState::Submitting
                ) && row.next_attempt_at <= now
                    && (row.claim_token.is_none() || row.claim_expires_at <= now)
            })
            .collect();
        candidates.sort_by(|left, right| left.next_attempt_at.cmp(&right.next_attempt_at));
        let account_key = candidates
            .first()
            .map(|row| (row.topic_id, row.system_mail_id.is_some()));
        let selected: Vec<_> = candidates
            .drain(..)
            .filter(|row| Some((row.topic_id, row.system_mail_id.is_some())) == account_key)
            .take(limit)
            .collect();
        let mut claimed = Vec::with_capacity(selected.len());
        for mut row in selected {
            row.claim_token = Some(batch_token.clone());
            row.claim_expires_at = now + TimeDuration::from_micros(CLAIM_LEASE_MICROS);
            row.attempt_count = row.attempt_count.saturating_add(1);
            row.updated_at = now;
            if row.state == JmapDeliveryState::Submitting {
                row.state = JmapDeliveryState::SubmissionUnknown;
            }
            tx.db
                .jmap_mail_deliveries()
                .delivery_id()
                .update(row.clone());
            claimed.push((row, batch_token.clone()));
        }
        claimed
    }))
}

fn fail_ingress(
    ctx: &mut ProcedureContext,
    ingress: &MailIngress,
    error: &str,
) -> Result<(), String> {
    let caller = ctx.sender();
    let id = ingress.id.clone();
    let detail = error.to_string();
    let now = ctx.timestamp;
    ctx.with_tx(|tx| {
        if let Some(mut current) = tx.db.mail_ingress().id().find(&id) {
            if current.claim.claim_owner == Some(caller) {
                current.claim.status = crate::common::delivery_types::DeliveryStatus::Failed;
                current.claim.last_error = Some(detail.clone());
                current.claim.claim_owner = None;
                current.claim.instance_id = None;
                current.claim.claim_expires_at = Timestamp::UNIX_EPOCH;
                current.claim.updated_at = now;
                current.completed_at = now;
                tx.db.mail_ingress().id().update(current);
            }
        }
    });
    Ok(())
}

fn uuid_token(ctx: &ProcedureContext) -> Result<String, String> {
    ctx.new_uuid_v4()
        .map(|value| value.to_string().replace('-', ""))
        .map_err(|error| format!("Could not generate delivery token: {error}"))
}

fn record_failure(
    ctx: &mut ProcedureContext,
    delivery: &JmapMailDelivery,
    claim_token: &str,
    error: jmap::DispatchError,
) -> Result<(), String> {
    let now = ctx.timestamp;
    let id = delivery.delivery_id.clone();
    let token = claim_token.to_string();
    ctx.with_tx(|tx| {
        let Some(mut current) = tx.db.jmap_mail_deliveries().delivery_id().find(&id) else {
            return;
        };
        if current.claim_token.as_deref() != Some(token.as_str()) {
            return;
        }
        current.last_error = Some(error.message.chars().take(500).collect());
        current.claim_token = None;
        current.claim_expires_at = Timestamp::UNIX_EPOCH;
        current.updated_at = now;
        if error.permanent {
            current.state = JmapDeliveryState::Failed;
        } else {
            current.state = if current.jmap_email_id.is_some() {
                JmapDeliveryState::SubmissionUnknown
            } else {
                JmapDeliveryState::Pending
            };
            let exponent = current.attempt_count.min(7);
            let seconds = (30u64.saturating_mul(1u64 << exponent)).min(3600);
            let jitter = u64::from(blake3::hash(id.as_bytes()).as_bytes()[0]) % 15;
            current.next_attempt_at =
                now + TimeDuration::from_micros(((seconds + jitter) as i64) * 1_000_000);
        }
        let event_type = if error.permanent {
            "jmap_failed"
        } else {
            "jmap_retry_scheduled"
        };
        tx.db
            .jmap_mail_delivery_events()
            .insert(JmapMailDeliveryEvent {
                id: 0,
                delivery_id: id.clone(),
                occurred_at: now,
                event_type: event_type.to_string(),
                attempt_no: current.attempt_count,
                jmap_email_id: current.jmap_email_id.clone(),
                jmap_submission_id: current.jmap_submission_id.clone(),
                details: error.message.chars().take(500).collect(),
                actor: JmapDeliveryActor::Scheduled,
            });
        tx.db.jmap_mail_deliveries().delivery_id().update(current);
    });
    Ok(())
}
