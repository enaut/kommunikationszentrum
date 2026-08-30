use spacetimedb::{
    Identity, Query, ReducerContext, ScheduleAt, SpacetimeType, Table, TimeDuration, Timestamp,
    ViewContext,
};

use crate::account::{is_admin_identity, is_admin_user};
use crate::mail_message::mail_message;

pub const MAX_INGRESS_ATTEMPTS: u32 = 3;

const MICROS_PER_SEC: i64 = 1_000_000;
const MICROS_PER_MIN: i64 = 60 * MICROS_PER_SEC;

fn ingress_lease_duration() -> TimeDuration {
    TimeDuration::from_micros(10 * MICROS_PER_MIN)
}

fn delivery_lease_duration() -> TimeDuration {
    TimeDuration::from_micros(5 * MICROS_PER_MIN)
}

fn ingress_retry_backoff(attempt_count: u32) -> TimeDuration {
    match attempt_count {
        1 => TimeDuration::from_micros(30 * MICROS_PER_SEC),
        2 => TimeDuration::from_micros(2 * MICROS_PER_MIN),
        3 => TimeDuration::from_micros(10 * MICROS_PER_MIN),
        _ => TimeDuration::from_micros(30 * MICROS_PER_MIN),
    }
}

/// State values shared between the ingress and delivery pipelines.
#[derive(Clone, PartialEq, Eq, SpacetimeType)]
pub enum DeliveryStatus {
    // Ingress states
    Pending,
    Processing,
    RetryScheduled,
    Completed,
    Failed,
    // Delivery states
    Queued,
    Sending,
    Sent,
    Bounced,
}

impl DeliveryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::RetryScheduled => "retry_scheduled",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Bounced => "bounced",
        }
    }
}

impl std::fmt::Display for DeliveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Claim/lease state embedded in queue rows.
#[derive(Clone, SpacetimeType)]
pub struct ClaimState {
    pub status: DeliveryStatus,
    pub next_attempt_at: Timestamp,
    pub claim_owner: Option<Identity>,
    pub instance_id: Option<String>,
    pub claim_expires_at: Timestamp,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub updated_at: Timestamp,
}

impl ClaimState {
    pub fn new_pending(now: Timestamp) -> Self {
        Self {
            status: DeliveryStatus::Pending,
            next_attempt_at: now,
            claim_owner: None,
            instance_id: None,
            claim_expires_at: Timestamp::UNIX_EPOCH,
            attempt_count: 0,
            last_error: None,
            updated_at: now,
        }
    }
}

/// One row per inbound message fan-out job.
/// Private: clients never subscribe to this table directly. `sender_mail_ingress`
/// below is the only way clients can read ingress rows.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_ingress)]
pub struct MailIngress {
    #[primary_key]
    pub id: String,
    /// FK → MailMessage.id
    #[index(btree)]
    pub mail_message_id: u64,
    #[index(btree)]
    pub category_id: u64,
    pub category_email: String,
    pub claim: ClaimState,
    pub recipient_count: u32,
    pub delivery_count: u32,
    pub failed_delivery_count: u32,
    pub completed_at: Timestamp,
}

/// Shared row type for all delivery-phase tables.
#[derive(Clone, SpacetimeType)]
pub struct MailDeliveryRow {
    pub id: String,
    /// FK → MailIngress.id
    pub ingress_id: String,
    /// FK → MailMessage.id
    pub mail_message_id: u64,
    pub category_id: u64,
    pub subscription_id: u64,
    pub recipient_email: String,
    pub recipient_account_id: Option<u64>,
    pub list_email: String,
    pub list_name: String,
    pub original_sender_email: String,
    /// Rewritten From header for outbound delivery
    pub from_header: String,
    pub reply_to: String,
    /// Final assembled SMTP envelope (complete RFC 5322 message bytes)
    pub raw_message: String,
    pub unsubscribe_token: String,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub smtp_status_code: Option<u16>,
    pub smtp_response: Option<String>,
    /// Set when the delivery reaches a terminal state (sent, failed, or bounced).
    pub finalized_at: Timestamp,
    pub updated_at: Timestamp,
}

/// Work items available for the sender worker to claim.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_pending)]
pub struct MailDeliveryPending {
    #[primary_key]
    pub id: String,
    #[index(btree)]
    pub ingress_id: String,
    pub row: MailDeliveryRow,
}

/// Temporary retry queue for transient SMTP failures.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_temporary_failed)]
pub struct MailDeliveryTemporaryFailed {
    #[primary_key]
    pub id: String,
    #[index(btree)]
    pub next_attempt_at: Timestamp,
    pub fail_reason: String,
    pub row: MailDeliveryRow,
}

/// Work items currently held by a worker (lease active).
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_claimed)]
pub struct MailDeliveryClaimed {
    #[primary_key]
    pub id: String,
    pub worker: Identity,
    pub instance_id: String,
    #[index(btree)]
    pub lease_expires_at: Timestamp,
    pub row: MailDeliveryRow,
}

/// Terminal rows — sent, failed, or bounced. Immutable after insert.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_done)]
pub struct MailDeliveryDone {
    #[primary_key]
    pub id: String,
    #[index(btree)]
    pub ingress_id: String,
    /// "sent" | "failed" | "bounced"
    pub final_state: String,
    pub row: MailDeliveryRow,
}

/// Schedule table for the `expire_stale_delivery_claims` reducer.
#[derive(Clone)]
#[spacetimedb::table(
    accessor = expire_stale_delivery_claims_schedule,
    public,
    scheduled(expire_stale_delivery_claims)
)]
pub struct ExpireStaleDeliveryClaimsSchedule {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}

/// Schedule table for the `requeue_temporary_failed_mails` reducer.
#[derive(Clone)]
#[spacetimedb::table(
    accessor = requeue_temporary_failed_mails_schedule,
    public,
    scheduled(requeue_temporary_failed_mails)
)]
pub struct RequeueTemporaryFailedMailsSchedule {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}

// Private: internal delivery audit log. Not exposed to any client view.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_events)]
pub struct MailDeliveryEvent {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub delivery_id: String,
    #[index(btree)]
    pub occurred_at: Timestamp,
    pub event_type: String,
    pub attempt_no: u32,
    pub smtp_status_code: Option<u16>,
    pub smtp_response: Option<String>,
    pub error_kind: Option<String>,
    pub details: String,
    pub worker_identity: Option<Identity>,
}

/// Full mail-ingress fan-out queue. Restricted to admins (the `sender`
/// service connects with an admin identity); everyone else gets an empty list.
#[spacetimedb::view(accessor = sender_mail_ingress, public)]
pub fn sender_mail_ingress(ctx: &ViewContext) -> impl Query<MailIngress> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_ingress().r#filter(move |_| is_admin)
}

/// Active delivery queue — for the sender worker (admin only).
#[spacetimedb::view(accessor = sender_mail_delivery_pending, public)]
pub fn sender_mail_delivery_pending(ctx: &ViewContext) -> impl Query<MailDeliveryPending> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_delivery_pending().r#filter(move |_| is_admin)
}

/// Claimed deliveries held by active workers (admin only).
#[spacetimedb::view(accessor = sender_mail_delivery_claimed, public)]
pub fn sender_mail_delivery_claimed(ctx: &ViewContext) -> impl Query<MailDeliveryClaimed> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_delivery_claimed().r#filter(move |_| is_admin)
}

/// Terminal delivery records — for admin audit (admin only).
#[spacetimedb::view(accessor = sender_mail_delivery_done, public)]
pub fn sender_mail_delivery_done(ctx: &ViewContext) -> impl Query<MailDeliveryDone> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_delivery_done().r#filter(move |_| is_admin)
}

/// Delivery event audit log — for admin troubleshooting and observability.
#[spacetimedb::view(accessor = sender_mail_delivery_events, public)]
pub fn sender_mail_delivery_events(ctx: &ViewContext) -> impl Query<MailDeliveryEvent> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_delivery_events().r#filter(move |_| is_admin)
}

/// Temporarily failed deliveries waiting for retry – admin only.
#[spacetimedb::view(accessor = sender_mail_delivery_temporary_failed, public)]
pub fn sender_mail_delivery_temporary_failed(
    ctx: &ViewContext,
) -> impl Query<MailDeliveryTemporaryFailed> {
    let is_admin = is_admin_user(ctx);
    ctx.from
        .mail_delivery_temporary_failed()
        .r#filter(move |_| is_admin)
}

fn make_ingress_id(ctx: &ReducerContext, queue_id: &str, category_id: u64) -> String {
    let entropy = ctx.random::<u128>();
    format!("{queue_id}:{category_id}:{entropy:032x}")
}

pub(crate) fn make_delivery_id(
    ingress_id: &str,
    subscription_id: u64,
    recipient_email: &str,
) -> String {
    format!("{ingress_id}:{subscription_id}:{recipient_email}")
}

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

    // TODO: this guard is always true because make_ingress_id includes entropy;
    // consider removing entropy from the ID if true idempotency is needed.
    if ctx.db.mail_ingress().id().find(&ingress_id).is_none() {
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
    }

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
    ingress: &MailIngress,
    subscription_id: u64,
    recipient_email: String,
    recipient_account_id: Option<u64>,
    list_email: String,
    list_name: String,
    original_sender_email: String,
    from_header: String,
    reply_to: String,
    raw_message: String,
    unsubscribe_token: String,
) -> String {
    let delivery_id = make_delivery_id(&ingress.id, subscription_id, &recipient_email);

    if ctx
        .db
        .mail_delivery_done()
        .id()
        .find(&delivery_id)
        .is_some()
    {
        return delivery_id;
    }

    if ctx
        .db
        .mail_delivery_claimed()
        .id()
        .find(&delivery_id)
        .is_some()
    {
        return delivery_id;
    }

    if let Some(mut existing) = ctx.db.mail_delivery_pending().id().find(&delivery_id) {
        debug_assert_eq!(existing.row.ingress_id, ingress.id);
        debug_assert_eq!(existing.row.subscription_id, subscription_id);
        debug_assert_eq!(existing.row.recipient_email, recipient_email);

        existing.row.recipient_account_id = recipient_account_id;
        existing.row.list_email = list_email;
        existing.row.list_name = list_name;
        existing.row.original_sender_email = original_sender_email;
        existing.row.from_header = from_header;
        existing.row.reply_to = reply_to;
        existing.row.raw_message = raw_message;
        existing.row.unsubscribe_token = unsubscribe_token;
        existing.row.updated_at = ctx.timestamp;
        ctx.db.mail_delivery_pending().id().update(existing);
        return delivery_id;
    }

    let row = MailDeliveryRow {
        id: delivery_id.clone(),
        ingress_id: ingress.id.clone(),
        mail_message_id: ingress.mail_message_id,
        category_id: ingress.category_id,
        subscription_id,
        recipient_email,
        recipient_account_id,
        list_email,
        list_name,
        original_sender_email,
        from_header,
        reply_to,
        raw_message,
        unsubscribe_token,
        attempt_count: 0,
        last_error: None,
        smtp_status_code: None,
        smtp_response: None,
        finalized_at: Timestamp::UNIX_EPOCH,
        updated_at: ctx.timestamp,
    };

    ctx.db.mail_delivery_pending().insert(MailDeliveryPending {
        id: delivery_id.clone(),
        ingress_id: ingress.id.clone(),
        row,
    });

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
    list_email: String,
    list_name: String,
    original_sender_email: String,
    from_header: String,
    reply_to: String,
    raw_message: String,
    unsubscribe_token: String,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let Some(ingress) = ctx.db.mail_ingress().id().find(&ingress_id) else {
        return Err(format!("Mail ingress '{ingress_id}' not found"));
    };
    upsert_mail_delivery(
        ctx,
        &ingress,
        subscription_id,
        recipient_email,
        recipient_account_id,
        list_email,
        list_name,
        original_sender_email,
        from_header,
        reply_to,
        raw_message,
        unsubscribe_token,
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
        .id()
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
    candidates.sort_by(|a, b| a.row.id.cmp(&b.row.id));

    let Some(pending) = candidates.into_iter().next() else {
        return Ok(());
    };

    ctx.db.mail_delivery_pending().id().delete(&pending.id);
    ctx.db.mail_delivery_claimed().insert(MailDeliveryClaimed {
        id: pending.id.clone(),
        worker: ctx.sender(),
        instance_id,
        lease_expires_at: ctx.timestamp + delivery_lease_duration(),
        row: MailDeliveryRow {
            updated_at: ctx.timestamp,
            ..pending.row
        },
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
        attempt_no: claimed.row.attempt_count,
        smtp_status_code,
        smtp_response: Some(smtp_response.clone()),
        error_kind: None,
        details: "Delivery accepted by SMTP server".to_string(),
        worker_identity: Some(ctx.sender()),
    });

    ctx.db.mail_delivery_claimed().id().delete(&delivery_id);
    ctx.db.mail_delivery_done().insert(MailDeliveryDone {
        id: delivery_id.clone(),
        ingress_id: claimed.row.ingress_id.clone(),
        final_state: DeliveryStatus::Sent.to_string(),
        row: MailDeliveryRow {
            smtp_status_code,
            smtp_response: Some(smtp_response),
            last_error: None,
            finalized_at: ctx.timestamp,
            updated_at: ctx.timestamp,
            ..claimed.row
        },
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn schedule_mail_delivery_retry(
    ctx: &ReducerContext,
    delivery_id: String,
    error_msg: String,
    delay_micros: i64,
) -> Result<(), String> {
    let Some(claimed) = ctx.db.mail_delivery_claimed().id().find(&delivery_id) else {
        return Err("Claimed delivery not found".to_string());
    };

    ctx.db.mail_delivery_claimed().id().delete(&delivery_id);

    let mut updated_row = claimed.row;
    updated_row.attempt_count = updated_row.attempt_count.saturating_add(1);
    updated_row.last_error = Some(error_msg.clone());
    updated_row.updated_at = ctx.timestamp;

    ctx.db
        .mail_delivery_temporary_failed()
        .insert(MailDeliveryTemporaryFailed {
            id: delivery_id,
            next_attempt_at: ctx.timestamp + TimeDuration::from_micros(delay_micros),
            fail_reason: error_msg,
            row: updated_row,
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
            .id()
            .delete(&failed_job.id);

        ctx.db.mail_delivery_pending().insert(MailDeliveryPending {
            id: failed_job.id.clone(),
            ingress_id: failed_job.row.ingress_id.clone(),
            row: failed_job.row,
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
        .id()
        .find(&delivery_id)
        .ok_or_else(|| format!("Temporary failed delivery '{delivery_id}' not found"))?;

    ctx.db
        .mail_delivery_temporary_failed()
        .id()
        .delete(&delivery_id);

    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id: delivery_id.clone(),
        occurred_at: ctx.timestamp,
        event_type: "retry_cancelled".to_string(),
        attempt_no: failed.row.attempt_count,
        smtp_status_code: failed.row.smtp_status_code,
        smtp_response: failed.row.smtp_response.clone(),
        error_kind: Some("manual_cancel".to_string()),
        details: "Temporary retry cancelled by admin".to_string(),
        worker_identity: Some(ctx.sender()),
    });

    ctx.db.mail_delivery_done().insert(MailDeliveryDone {
        id: delivery_id.clone(),
        ingress_id: failed.row.ingress_id.clone(),
        final_state: "cancelled".to_string(),
        row: MailDeliveryRow {
            last_error: Some("Retry cancelled by admin".to_string()),
            finalized_at: ctx.timestamp,
            updated_at: ctx.timestamp,
            ..failed.row
        },
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
        attempt_no: claimed.row.attempt_count,
        smtp_status_code,
        smtp_response: Some(smtp_response.clone()),
        error_kind: Some(error_kind),
        details: format!("SMTP delivery failed: {smtp_response}"),
        worker_identity: Some(ctx.sender()),
    });

    ctx.db.mail_delivery_claimed().id().delete(&delivery_id);
    ctx.db.mail_delivery_done().insert(MailDeliveryDone {
        id: delivery_id.clone(),
        ingress_id: claimed.row.ingress_id.clone(),
        final_state: DeliveryStatus::Failed.to_string(),
        row: MailDeliveryRow {
            smtp_status_code,
            smtp_response: Some(smtp_response.clone()),
            last_error: Some(smtp_response),
            finalized_at: ctx.timestamp,
            updated_at: ctx.timestamp,
            ..claimed.row
        },
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
        attempt_no: claimed.row.attempt_count,
        smtp_status_code: status_code,
        smtp_response: Some(smtp_response.clone()),
        error_kind: Some(error_kind),
        details: format!("Delivery bounced: {smtp_response}"),
        worker_identity: Some(ctx.sender()),
    });

    ctx.db.mail_delivery_claimed().id().delete(&delivery_id);
    ctx.db.mail_delivery_done().insert(MailDeliveryDone {
        id: delivery_id.clone(),
        ingress_id: claimed.row.ingress_id.clone(),
        final_state: DeliveryStatus::Bounced.to_string(),
        row: MailDeliveryRow {
            smtp_status_code: status_code,
            smtp_response: Some(smtp_response.clone()),
            last_error: Some(smtp_response),
            finalized_at: ctx.timestamp,
            updated_at: ctx.timestamp,
            ..claimed.row
        },
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
        ctx.db.mail_delivery_claimed().id().delete(&claimed.id);
        ctx.db.mail_delivery_pending().insert(MailDeliveryPending {
            id: claimed.id.clone(),
            ingress_id: claimed.row.ingress_id.clone(),
            row: MailDeliveryRow {
                last_error: Some("Lease expired — requeued".to_string()),
                updated_at: now,
                ..claimed.row
            },
        });
        ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
            id: 0,
            delivery_id: claimed.id,
            occurred_at: now,
            event_type: "lease_expired".to_string(),
            attempt_no: claimed.row.attempt_count,
            smtp_status_code: None,
            smtp_response: None,
            error_kind: Some("lease_timeout".to_string()),
            details: "Delivery lease expired; requeued for retry".to_string(),
            worker_identity: Some(claimed.worker),
        });
    }
    Ok(())
}
