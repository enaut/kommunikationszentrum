use spacetimedb::{ReducerContext, Table, TimeDuration, Timestamp};

use crate::common::auth::is_admin_identity;
use crate::common::delivery_types::*;
use crate::common::id::*;
use crate::models::delivery::*;
use crate::models::mail_message::mail_message;

pub(crate) fn upsert_mail_ingress(
    ctx: &ReducerContext,
    mail_message_id: u64,
    category_id: u64,
    category_email: String,
) -> String {
    let msg = ctx
        .db
        .mail_message()
        .id()
        .find(&mail_message_id)
        .expect("MailMessage must exist before MailIngress");
    let queue_id = msg.queue_id.as_deref().unwrap_or("");
    let ingress_id = make_ingress_id(ctx, queue_id, category_id);

    ctx.db.mail_ingress().insert(MailIngress {
        id: ingress_id.clone(),
        mail_message_id,
        category_id,
        category_email,
        claim: ClaimState::new_pending(ctx.timestamp),
        recipient_count: 0,
        delivery_count: 0,
        failed_delivery_count: 0,
        completed_at: Timestamp::UNIX_EPOCH,
    });

    ingress_id
}

fn claimable_ingress(row: &MailIngress, now: Timestamp) -> bool {
    let status_claimable = match row.claim.status {
        DeliveryStatus::Pending | DeliveryStatus::RetryScheduled => {
            row.claim.claim_owner.is_none() || row.claim.claim_expires_at <= now
        }
        DeliveryStatus::Processing => row.claim.claim_expires_at <= now,
        _ => false,
    };
    status_claimable && row.claim.next_attempt_at <= now
}

#[spacetimedb::reducer]
pub fn claim_next_mail_ingress(ctx: &ReducerContext, instance_id: String) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }

    let mut candidates: Vec<MailIngress> = ctx
        .db
        .mail_ingress()
        .iter()
        .filter(|row| claimable_ingress(row, ctx.timestamp))
        .collect();

    candidates.sort_by(|left, right| {
        left.claim
            .next_attempt_at
            .cmp(&right.claim.next_attempt_at)
            .then(left.id.cmp(&right.id))
    });

    let Some(mut row) = candidates.into_iter().next() else {
        return Ok(());
    };

    row.claim.status = DeliveryStatus::Processing;
    row.claim.claim_owner = Some(ctx.sender());
    row.claim.instance_id = Some(instance_id);
    row.claim.claim_expires_at = ctx.timestamp + ingress_lease_duration();
    row.claim.attempt_count = row.claim.attempt_count.saturating_add(1);
    row.claim.next_attempt_at = ctx.timestamp;
    row.claim.last_error = None;
    row.claim.updated_at = ctx.timestamp;

    ctx.db.mail_ingress().id().update(row);
    Ok(())
}

fn require_ingress_claimed_by_me(
    ctx: &ReducerContext,
    ingress_id: &str,
    instance_id: &str,
) -> Result<MailIngress, String> {
    let row = ctx
        .db
        .mail_ingress()
        .id()
        .find(&ingress_id.to_string())
        .ok_or_else(|| format!("Mail ingress '{ingress_id}' not found"))?;

    if row.claim.claim_owner != Some(ctx.sender()) {
        return Err(format!(
            "Mail ingress '{ingress_id}' is not owned by {:?}",
            ctx.sender()
        ));
    }
    if row.claim.instance_id.as_deref() != Some(instance_id) {
        return Err(format!(
            "Mail ingress '{ingress_id}' is claimed by a different worker instance"
        ));
    }
    Ok(row)
}

#[spacetimedb::reducer]
pub fn increment_mail_ingress_delivery_count(
    ctx: &ReducerContext,
    ingress_id: String,
    instance_id: String,
) -> Result<(), String> {
    let mut ingress = ctx
        .db
        .mail_ingress()
        .id()
        .find(&ingress_id)
        .ok_or_else(|| format!("MailIngress {} not found", ingress_id))?;

    if ingress.claim.instance_id != Some(instance_id.to_string()) {
        return Err(format!(
            "MailIngress {} not claimed by {}",
            ingress_id, instance_id
        ));
    }

    ingress.delivery_count = ingress.delivery_count.saturating_add(1);
    ctx.db.mail_ingress().id().update(ingress);
    Ok(())
}

#[spacetimedb::reducer]
pub fn increment_mail_ingress_failed_delivery_count(
    ctx: &ReducerContext,
    ingress_id: String,
    instance_id: String,
) -> Result<(), String> {
    let mut ingress = ctx
        .db
        .mail_ingress()
        .id()
        .find(&ingress_id)
        .ok_or_else(|| format!("MailIngress {} not found", ingress_id))?;

    if ingress.claim.instance_id != Some(instance_id.to_string()) {
        return Err(format!(
            "MailIngress {} not claimed by {}",
            ingress_id, instance_id
        ));
    }

    ingress.failed_delivery_count = ingress.failed_delivery_count.saturating_add(1);
    ctx.db.mail_ingress().id().update(ingress);
    Ok(())
}

#[spacetimedb::reducer]
pub fn complete_mail_ingress(
    ctx: &ReducerContext,
    ingress_id: String,
    instance_id: String,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let mut row = require_ingress_claimed_by_me(ctx, &ingress_id, &instance_id)?;

    row.claim.status = DeliveryStatus::Completed;
    row.claim.last_error = None;
    row.claim.claim_owner = None;
    row.claim.instance_id = None;
    row.claim.claim_expires_at = Timestamp::UNIX_EPOCH;
    row.completed_at = ctx.timestamp;
    row.claim.updated_at = ctx.timestamp;
    ctx.db.mail_ingress().id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn retry_mail_ingress(
    ctx: &ReducerContext,
    ingress_id: String,
    instance_id: String,
    error: String,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let mut row = require_ingress_claimed_by_me(ctx, &ingress_id, &instance_id)?;

    row.claim.last_error = Some(error);
    row.claim.claim_owner = None;
    row.claim.instance_id = None;
    row.claim.claim_expires_at = Timestamp::UNIX_EPOCH;
    row.claim.updated_at = ctx.timestamp;

    if row.claim.attempt_count >= MAX_INGRESS_ATTEMPTS {
        row.claim.status = DeliveryStatus::Failed;
        row.completed_at = ctx.timestamp;
    } else {
        row.claim.status = DeliveryStatus::RetryScheduled;
        row.claim.next_attempt_at = ctx.timestamp + ingress_retry_backoff(row.claim.attempt_count);
    }

    ctx.db.mail_ingress().id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn fail_mail_ingress(
    ctx: &ReducerContext,
    ingress_id: String,
    instance_id: String,
    error: String,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let mut row = require_ingress_claimed_by_me(ctx, &ingress_id, &instance_id)?;

    row.claim.status = DeliveryStatus::Failed;
    row.claim.last_error = Some(error);
    row.claim.claim_owner = None;
    row.claim.instance_id = None;
    row.claim.claim_expires_at = Timestamp::UNIX_EPOCH;
    row.completed_at = ctx.timestamp;
    row.claim.updated_at = ctx.timestamp;
    ctx.db.mail_ingress().id().update(row);
    Ok(())
}

pub(crate) fn upsert_mail_delivery(
    ctx: &ReducerContext,
    ingress: &mut MailIngress,
    subscription_id: u64,
    recipient_email: String,
    recipient_account_id: Option<u64>,
    from_header: String,
    reply_to: String,
    raw_message: String,
) -> String {
    let delivery_id = make_delivery_id(&ingress.id, subscription_id, &recipient_email);

    // Ensure the immutable MailDeliveryMessage exists
    if ctx
        .db
        .mail_delivery_message()
        .delivery_id()
        .find(&delivery_id)
        .is_none()
    {
        ctx.db.mail_delivery_message().insert(MailDeliveryMessage {
            delivery_id: delivery_id.clone(),
            mail_message_id: ingress.mail_message_id,
            ingress_id: ingress.id.clone(),
            subscription_id,
            recipient_email,
            recipient_account_id,
            from_header,
            reply_to,
            raw_message,
        });
    }

    // Do not re-enqueue if already terminal, claimed, or awaiting retry
    if ctx
        .db
        .mail_delivery_done()
        .delivery_id()
        .find(&delivery_id)
        .is_some()
        || ctx
            .db
            .mail_delivery_claimed()
            .delivery_id()
            .find(&delivery_id)
            .is_some()
        || ctx
            .db
            .mail_delivery_temporary_failed()
            .delivery_id()
            .find(&delivery_id)
            .is_some()
    {
        return delivery_id;
    }

    if let Some(mut existing) = ctx
        .db
        .mail_delivery_pending()
        .delivery_id()
        .find(&delivery_id)
    {
        existing.last_updated = ctx.timestamp;
        ctx.db
            .mail_delivery_pending()
            .delivery_id()
            .update(existing);
        return delivery_id;
    }

    ctx.db.mail_delivery_pending().insert(MailDeliveryPending {
        delivery_id: delivery_id.clone(),
        ingress_id: ingress.id.clone(),
        attempt_count: 0,
        last_error: None,
        last_updated: ctx.timestamp,
    });

    ingress.delivery_count = ingress.delivery_count.saturating_add(1);
    ingress.recipient_count = ingress.recipient_count.saturating_add(1);
    ctx.db.mail_ingress().id().update(ingress.clone());

    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id: delivery_id.clone(),
        occurred_at: ctx.timestamp,
        event_type: DeliveryStatus::Queued.to_string(),
        attempt_no: 0,
        smtp_status_code: None,
        smtp_response: None,
        error_kind: None,
        details: "Delivery queued for SMTP submission".to_string(),
        worker_identity: Some(ctx.sender()),
    });

    delivery_id
}

#[spacetimedb::reducer]
pub fn enqueue_mail_delivery(
    ctx: &ReducerContext,
    ingress_id: String,
    subscription_id: u64,
    recipient_email: String,
    recipient_account_id: Option<u64>,
    from_header: String,
    reply_to: String,
    raw_message: String,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let Some(mut ingress) = ctx.db.mail_ingress().id().find(&ingress_id) else {
        return Err(format!("Mail ingress '{ingress_id}' not found"));
    };
    upsert_mail_delivery(
        ctx,
        &mut ingress,
        subscription_id,
        recipient_email,
        recipient_account_id,
        from_header,
        reply_to,
        raw_message,
    );
    Ok(())
}

fn require_claimed_by_me(
    ctx: &ReducerContext,
    delivery_id: &str,
    instance_id: &str,
) -> Result<MailDeliveryClaimed, String> {
    let row = ctx
        .db
        .mail_delivery_claimed()
        .delivery_id()
        .find(&delivery_id.to_string())
        .ok_or_else(|| format!("Mail delivery '{delivery_id}' not found or not claimed"))?;

    if row.worker != ctx.sender() {
        return Err(format!(
            "Mail delivery '{delivery_id}' is claimed by a different worker identity"
        ));
    }
    if row.instance_id != instance_id {
        return Err(format!(
            "Mail delivery '{delivery_id}' is claimed by a different worker instance"
        ));
    }
    Ok(row)
}

#[spacetimedb::reducer]
pub fn claim_next_mail_delivery(ctx: &ReducerContext, instance_id: String) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }

    let mut candidates: Vec<MailDeliveryPending> = ctx.db.mail_delivery_pending().iter().collect();
    candidates.sort_by(|a, b| a.delivery_id.cmp(&b.delivery_id));

    let Some(pending) = candidates.into_iter().next() else {
        return Ok(());
    };

    ctx.db
        .mail_delivery_pending()
        .delivery_id()
        .delete(&pending.delivery_id);
    ctx.db.mail_delivery_claimed().insert(MailDeliveryClaimed {
        delivery_id: pending.delivery_id,
        ingress_id: pending.ingress_id,
        worker: ctx.sender(),
        instance_id,
        lease_expires_at: ctx.timestamp + delivery_lease_duration(),
        attempt_count: pending.attempt_count,
        last_error: pending.last_error,
        last_updated: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn mark_mail_delivery_sent(
    ctx: &ReducerContext,
    delivery_id: String,
    instance_id: String,
    smtp_status_code: Option<u16>,
    smtp_response: String,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let claimed = require_claimed_by_me(ctx, &delivery_id, &instance_id)?;

    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id: delivery_id.clone(),
        occurred_at: ctx.timestamp,
        event_type: DeliveryStatus::Sent.to_string(),
        attempt_no: claimed.attempt_count,
        smtp_status_code,
        smtp_response: Some(smtp_response.clone()),
        error_kind: None,
        details: "Delivery accepted by SMTP server".to_string(),
        worker_identity: Some(ctx.sender()),
    });

    ctx.db
        .mail_delivery_claimed()
        .delivery_id()
        .delete(&delivery_id);
    ctx.db.mail_delivery_done().insert(MailDeliveryDone {
        delivery_id,
        ingress_id: claimed.ingress_id,
        final_state: DeliveryFinalState::Sent,
        attempt_count: claimed.attempt_count,
        last_error: None,
        smtp_status_code,
        smtp_response: Some(smtp_response),
        finalized_at: ctx.timestamp,
        last_updated: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn schedule_mail_delivery_retry(
    ctx: &ReducerContext,
    delivery_id: String,
    instance_id: String,
    error_msg: String,
    delay_micros: i64,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let claimed = require_claimed_by_me(ctx, &delivery_id, &instance_id)?;

    let next_attempt = claimed.attempt_count.saturating_add(1);

    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id: delivery_id.clone(),
        occurred_at: ctx.timestamp,
        event_type: "retry_scheduled".to_string(),
        attempt_no: next_attempt,
        smtp_status_code: None,
        smtp_response: Some(error_msg.clone()),
        error_kind: Some("temporary_failure".to_string()),
        details: format!("Delivery retry scheduled: {error_msg}"),
        worker_identity: Some(ctx.sender()),
    });

    ctx.db
        .mail_delivery_claimed()
        .delivery_id()
        .delete(&delivery_id);

    ctx.db
        .mail_delivery_temporary_failed()
        .insert(MailDeliveryTemporaryFailed {
            delivery_id,
            ingress_id: claimed.ingress_id,
            next_attempt_at: ctx.timestamp + TimeDuration::from_micros(delay_micros),
            attempt_count: next_attempt,
            last_error: error_msg,
            smtp_status_code: None,
            smtp_response: None,
            last_updated: ctx.timestamp,
        });

    Ok(())
}

#[spacetimedb::reducer]
pub fn requeue_temporary_failed_mails(
    ctx: &ReducerContext,
    _scheduled: RequeueTemporaryFailedMailsSchedule,
) -> Result<(), String> {
    let now = ctx.timestamp;

    let ready_to_retry: Vec<MailDeliveryTemporaryFailed> = ctx
        .db
        .mail_delivery_temporary_failed()
        .iter()
        .filter(|failed| failed.next_attempt_at <= now)
        .collect();

    for failed_job in ready_to_retry {
        ctx.db
            .mail_delivery_temporary_failed()
            .delivery_id()
            .delete(&failed_job.delivery_id);

        ctx.db.mail_delivery_pending().insert(MailDeliveryPending {
            delivery_id: failed_job.delivery_id.clone(),
            ingress_id: failed_job.ingress_id,
            attempt_count: failed_job.attempt_count,
            last_error: Some(failed_job.last_error.clone()),
            last_updated: now,
        });

        ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
            id: 0,
            delivery_id: failed_job.delivery_id,
            occurred_at: now,
            event_type: "retry_requeued".to_string(),
            attempt_no: failed_job.attempt_count,
            smtp_status_code: failed_job.smtp_status_code,
            smtp_response: failed_job.smtp_response,
            error_kind: None,
            details: "Temporary failure retry requeued to pending queue".to_string(),
            worker_identity: None,
        });
    }

    Ok(())
}

#[spacetimedb::reducer]
pub fn cancel_mail_delivery_retry(ctx: &ReducerContext, delivery_id: String) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }

    let failed = ctx
        .db
        .mail_delivery_temporary_failed()
        .delivery_id()
        .find(&delivery_id)
        .ok_or_else(|| format!("Temporary failed delivery '{delivery_id}' not found"))?;

    ctx.db
        .mail_delivery_temporary_failed()
        .delivery_id()
        .delete(&delivery_id);

    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id: delivery_id.clone(),
        occurred_at: ctx.timestamp,
        event_type: "retry_cancelled".to_string(),
        attempt_no: failed.attempt_count,
        smtp_status_code: failed.smtp_status_code,
        smtp_response: failed.smtp_response.clone(),
        error_kind: Some("manual_cancel".to_string()),
        details: "Temporary retry cancelled by admin".to_string(),
        worker_identity: Some(ctx.sender()),
    });

    ctx.db.mail_delivery_done().insert(MailDeliveryDone {
        delivery_id,
        ingress_id: failed.ingress_id,
        final_state: DeliveryFinalState::Cancelled,
        attempt_count: failed.attempt_count,
        last_error: Some("Retry cancelled by admin".to_string()),
        smtp_status_code: failed.smtp_status_code,
        smtp_response: failed.smtp_response,
        finalized_at: ctx.timestamp,
        last_updated: ctx.timestamp,
    });

    Ok(())
}

#[spacetimedb::reducer]
pub fn fail_mail_delivery(
    ctx: &ReducerContext,
    delivery_id: String,
    instance_id: String,
    smtp_status_code: Option<u16>,
    smtp_response: String,
    error_kind: String,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let claimed = require_claimed_by_me(ctx, &delivery_id, &instance_id)?;

    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id: delivery_id.clone(),
        occurred_at: ctx.timestamp,
        event_type: DeliveryStatus::Failed.to_string(),
        attempt_no: claimed.attempt_count,
        smtp_status_code,
        smtp_response: Some(smtp_response.clone()),
        error_kind: Some(error_kind),
        details: format!("SMTP delivery failed: {smtp_response}"),
        worker_identity: Some(ctx.sender()),
    });

    ctx.db
        .mail_delivery_claimed()
        .delivery_id()
        .delete(&delivery_id);
    ctx.db.mail_delivery_done().insert(MailDeliveryDone {
        delivery_id,
        ingress_id: claimed.ingress_id,
        final_state: DeliveryFinalState::Failed,
        attempt_count: claimed.attempt_count,
        last_error: Some(smtp_response.clone()),
        smtp_status_code,
        smtp_response: Some(smtp_response),
        finalized_at: ctx.timestamp,
        last_updated: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn mark_mail_delivery_bounced(
    ctx: &ReducerContext,
    delivery_id: String,
    instance_id: String,
    smtp_status_code: Option<u16>,
    smtp_response: String,
    error_kind: String,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let claimed = require_claimed_by_me(ctx, &delivery_id, &instance_id)?;
    let status_code = smtp_status_code.or(Some(550));

    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id: delivery_id.clone(),
        occurred_at: ctx.timestamp,
        event_type: DeliveryStatus::Bounced.to_string(),
        attempt_no: claimed.attempt_count,
        smtp_status_code: status_code,
        smtp_response: Some(smtp_response.clone()),
        error_kind: Some(error_kind),
        details: format!("Delivery bounced: {smtp_response}"),
        worker_identity: Some(ctx.sender()),
    });

    ctx.db
        .mail_delivery_claimed()
        .delivery_id()
        .delete(&delivery_id);
    ctx.db.mail_delivery_done().insert(MailDeliveryDone {
        delivery_id,
        ingress_id: claimed.ingress_id,
        final_state: DeliveryFinalState::Bounced,
        attempt_count: claimed.attempt_count,
        last_error: Some(smtp_response.clone()),
        smtp_status_code: status_code,
        smtp_response: Some(smtp_response),
        finalized_at: ctx.timestamp,
        last_updated: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn expire_stale_delivery_claims(
    ctx: &ReducerContext,
    _scheduled: ExpireStaleDeliveryClaimsSchedule,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let now = ctx.timestamp;

    // Reset stale ingress claims whose lease expired while in Processing state
    let stale_ingress: Vec<MailIngress> = ctx
        .db
        .mail_ingress()
        .iter()
        .filter(|row| {
            row.claim.status == DeliveryStatus::Processing && row.claim.claim_expires_at <= now
        })
        .collect();

    for mut ingress in stale_ingress {
        ingress.claim.status = DeliveryStatus::Pending;
        ingress.claim.claim_owner = None;
        ingress.claim.instance_id = None;
        ingress.claim.claim_expires_at = Timestamp::UNIX_EPOCH;
        ingress.claim.last_error = Some("Lease expired — requeued".to_string());
        ingress.claim.updated_at = now;
        ctx.db.mail_ingress().id().update(ingress);
    }

    let stale: Vec<MailDeliveryClaimed> = ctx
        .db
        .mail_delivery_claimed()
        .lease_expires_at()
        .filter(..=now)
        .collect();

    for claimed in stale {
        ctx.db
            .mail_delivery_claimed()
            .delivery_id()
            .delete(&claimed.delivery_id);
        ctx.db.mail_delivery_pending().insert(MailDeliveryPending {
            delivery_id: claimed.delivery_id.clone(),
            ingress_id: claimed.ingress_id,
            attempt_count: claimed.attempt_count,
            last_error: Some("Lease expired — requeued".to_string()),
            last_updated: now,
        });
        ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
            id: 0,
            delivery_id: claimed.delivery_id,
            occurred_at: now,
            event_type: "lease_expired".to_string(),
            attempt_no: claimed.attempt_count,
            smtp_status_code: None,
            smtp_response: None,
            error_kind: Some("lease_timeout".to_string()),
            details: "Delivery lease expired; requeued for retry".to_string(),
            worker_identity: Some(claimed.worker),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_delivery_id() {
        let id = make_delivery_id("ing-123", 42, "user@example.com");
        assert_eq!(id, "ing-123:42:user@example.com");
    }

    #[test]
    fn test_delivery_final_state_as_str() {
        assert_eq!(DeliveryFinalState::Sent.as_str(), "sent");
        assert_eq!(DeliveryFinalState::Failed.as_str(), "failed");
        assert_eq!(DeliveryFinalState::Bounced.as_str(), "bounced");
        assert_eq!(DeliveryFinalState::Cancelled.as_str(), "cancelled");

        assert_eq!(format!("{}", DeliveryFinalState::Sent), "sent");
        assert_eq!(format!("{}", DeliveryFinalState::Failed), "failed");
        assert_eq!(format!("{}", DeliveryFinalState::Bounced), "bounced");
        assert_eq!(format!("{}", DeliveryFinalState::Cancelled), "cancelled");
    }

    #[test]
    fn test_delivery_status_as_str() {
        assert_eq!(DeliveryStatus::Pending.as_str(), "pending");
        assert_eq!(DeliveryStatus::Processing.as_str(), "processing");
        assert_eq!(DeliveryStatus::RetryScheduled.as_str(), "retry_scheduled");
        assert_eq!(DeliveryStatus::Completed.as_str(), "completed");
        assert_eq!(DeliveryStatus::Failed.as_str(), "failed");
        assert_eq!(DeliveryStatus::Queued.as_str(), "queued");
        assert_eq!(DeliveryStatus::Sending.as_str(), "sending");
        assert_eq!(DeliveryStatus::Sent.as_str(), "sent");
        assert_eq!(DeliveryStatus::Bounced.as_str(), "bounced");
    }
}
