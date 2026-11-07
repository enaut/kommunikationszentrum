use spacetimedb::{ProcedureContext, ReducerContext, Table, Timestamp};

use crate::{
    common::auth::is_admin_identity,
    models::{
        delivery::{
            MailDeliveryPending, MailDeliveryTemporaryFailed, mail_delivery_claimed,
            mail_delivery_message, mail_delivery_pending, mail_delivery_temporary_failed,
            mail_ingress,
        },
        mail_message::mail_message,
        sender::{
            JmapDeliveryState, JmapMailDelivery, JmapMailDeliveryEvent, jmap_mail_deliveries,
            jmap_mail_delivery_events,
        },
    },
};

/// Move unclaimed legacy pending and temporary-failure rows into the JMAP queue.
/// Claimed legacy rows are deliberately left untouched because Stalwart may already have accepted them.
#[spacetimedb::procedure]
pub fn migrate_pending_legacy_deliveries(ctx: &mut ProcedureContext) -> Result<u32, String> {
    let caller = ctx.sender();
    let authorized = ctx.with_tx(|tx| is_admin_identity(tx, caller));
    if !authorized {
        return Err("Unauthorized: Admin access required".to_string());
    }

    #[derive(Clone)]
    struct LegacyJob {
        delivery_id: String,
        ingress_id: String,
        attempt_count: u32,
        next_attempt_at: Timestamp,
        last_error: Option<String>,
        temporary: bool,
    }

    let now = ctx.timestamp;
    let jobs = ctx.with_tx(|tx| {
        let pending: Vec<_> = tx
            .db
            .mail_delivery_pending()
            .iter()
            .map(|row: MailDeliveryPending| LegacyJob {
                delivery_id: row.delivery_id,
                ingress_id: row.ingress_id,
                attempt_count: row.attempt_count,
                next_attempt_at: now,
                last_error: row.last_error,
                temporary: false,
            })
            .collect();
        let failed: Vec<_> = tx
            .db
            .mail_delivery_temporary_failed()
            .iter()
            .map(|row: MailDeliveryTemporaryFailed| LegacyJob {
                delivery_id: row.delivery_id,
                ingress_id: row.ingress_id,
                attempt_count: row.attempt_count,
                next_attempt_at: row.next_attempt_at,
                last_error: Some(row.last_error),
                temporary: true,
            })
            .collect();
        pending.into_iter().chain(failed).collect::<Vec<_>>()
    });

    let mut migrated = 0u32;
    for job in jobs {
        let legacy_id = job.delivery_id.clone();
        let ingress_id = job.ingress_id.clone();
        let temporary = job.temporary;
        let next_attempt_at = job.next_attempt_at;
        let attempt_count = job.attempt_count;
        let last_error = job.last_error.clone();
        let inserted = ctx.with_tx(|tx| {
            let still_queued = if temporary {
                tx.db
                    .mail_delivery_temporary_failed()
                    .delivery_id()
                    .find(&legacy_id)
                    .is_some()
            } else {
                tx.db
                    .mail_delivery_pending()
                    .delivery_id()
                    .find(&legacy_id)
                    .is_some()
            };
            if !still_queued
                || tx
                    .db
                    .mail_delivery_claimed()
                    .delivery_id()
                    .find(&legacy_id)
                    .is_some()
            {
                return false;
            }
            let Some(message) = tx.db.mail_delivery_message().delivery_id().find(&legacy_id) else {
                return false;
            };
            let Some(ingress) = tx.db.mail_ingress().id().find(&ingress_id) else {
                return false;
            };
            let opaque_id = format!("legacy:{}", blake3::hash(legacy_id.as_bytes()).to_hex());
            if tx
                .db
                .jmap_mail_deliveries()
                .delivery_id()
                .find(&opaque_id)
                .is_none()
            {
                let subject = tx
                    .db
                    .mail_message()
                    .id()
                    .find(&message.mail_message_id)
                    .map(|canonical| canonical.subject)
                    .unwrap_or_default();
                tx.db.jmap_mail_deliveries().insert(JmapMailDelivery {
                    delivery_id: opaque_id.clone(),
                    ingress_id: Some(ingress_id.clone()),
                    topic_id: Some(ingress.topic_id),
                    subscription_id: Some(message.subscription_id),
                    system_mail_id: None,
                    legacy_delivery_id: Some(legacy_id.clone()),
                    mail_message_id: Some(message.mail_message_id),
                    recipient_email: message.recipient_email.clone(),
                    from_email: message.from_header.clone(),
                    subject,
                    body_text: String::new(),
                    state: JmapDeliveryState::Pending,
                    next_attempt_at,
                    attempt_count,
                    claim_owner: None,
                    claim_token: None,
                    claim_expires_at: Timestamp::UNIX_EPOCH,
                    jmap_email_id: None,
                    jmap_submission_id: None,
                    last_error: last_error.clone(),
                    created_at: now,
                    updated_at: now,
                });
                tx.db
                    .jmap_mail_delivery_events()
                    .insert(JmapMailDeliveryEvent {
                        id: 0,
                        delivery_id: opaque_id,
                        occurred_at: now,
                        event_type: "jmap_migrated".to_string(),
                        attempt_no: attempt_count,
                        jmap_email_id: None,
                        jmap_submission_id: None,
                        details: "Unclaimed legacy delivery migrated to native JMAP".to_string(),
                        worker_identity: Some(caller),
                    });
            }
            if temporary {
                tx.db
                    .mail_delivery_temporary_failed()
                    .delivery_id()
                    .delete(&legacy_id);
            } else {
                tx.db
                    .mail_delivery_pending()
                    .delivery_id()
                    .delete(&legacy_id);
            }
            true
        });
        if inserted {
            migrated = migrated.saturating_add(1);
        }
    }
    Ok(migrated)
}

/// Requeue a permanently failed native JMAP delivery after an administrator has corrected its cause.
#[spacetimedb::reducer]
pub fn retry_failed_jmap_delivery(ctx: &ReducerContext, delivery_id: String) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    let mut delivery = ctx
        .db
        .jmap_mail_deliveries()
        .delivery_id()
        .find(&delivery_id)
        .ok_or_else(|| format!("JMAP delivery '{delivery_id}' was not found"))?;
    if delivery.state != JmapDeliveryState::Failed {
        return Err("Only permanently failed JMAP deliveries can be retried manually".to_string());
    }
    delivery.state = if delivery.jmap_email_id.is_some() {
        JmapDeliveryState::SubmissionUnknown
    } else {
        JmapDeliveryState::Pending
    };
    delivery.next_attempt_at = ctx.timestamp;
    delivery.claim_owner = None;
    delivery.claim_token = None;
    delivery.claim_expires_at = Timestamp::UNIX_EPOCH;
    delivery.last_error = None;
    delivery.updated_at = ctx.timestamp;
    ctx.db
        .jmap_mail_deliveries()
        .delivery_id()
        .update(delivery.clone());
    ctx.db
        .jmap_mail_delivery_events()
        .insert(JmapMailDeliveryEvent {
            id: 0,
            delivery_id,
            occurred_at: ctx.timestamp,
            event_type: "jmap_manual_retry".to_string(),
            attempt_no: delivery.attempt_count,
            jmap_email_id: delivery.jmap_email_id,
            jmap_submission_id: delivery.jmap_submission_id,
            details: "Delivery manually requeued by an administrator".to_string(),
            worker_identity: Some(ctx.sender()),
        });
    Ok(())
}
