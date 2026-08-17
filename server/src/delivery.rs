use spacetimedb::{
    Identity, Query, ReducerContext, ScheduleAt, SpacetimeType, Table, TimeDuration, Timestamp,
    ViewContext,
};

use crate::account::{is_admin_identity, is_admin_user};
use crate::mail_message::mail_message;

pub const MAX_INGRESS_ATTEMPTS: u32 = 3;
pub const MAX_DELIVERY_ATTEMPTS: u32 = 5;

const MICROS_PER_SEC: i64 = 1_000_000;
const MICROS_PER_MIN: i64 = 60 * MICROS_PER_SEC;
const MICROS_PER_HOUR: i64 = 60 * MICROS_PER_MIN;

fn ingress_lease_duration() -> TimeDuration {
    TimeDuration::from_micros(10 * MICROS_PER_MIN)
}

fn delivery_lease_duration() -> TimeDuration {
    TimeDuration::from_micros(5 * MICROS_PER_MIN)
}

fn delivery_retry_backoff(attempt_count: u32) -> TimeDuration {
    match attempt_count {
        1 => TimeDuration::from_micros(30 * MICROS_PER_SEC),
        2 => TimeDuration::from_micros(2 * MICROS_PER_MIN),
        3 => TimeDuration::from_micros(10 * MICROS_PER_MIN),
        4 => TimeDuration::from_micros(30 * MICROS_PER_MIN),
        5 => TimeDuration::from_micros(60 * MICROS_PER_MIN),
        _ => TimeDuration::from_micros(12 * MICROS_PER_HOUR),
    }
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

/// Shared row type for all three delivery-phase tables.
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
    pub next_attempt_at: Timestamp,
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
    #[index(btree)]
    pub next_attempt_at: Timestamp,
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
    matches!(
        row.claim.status,
        DeliveryStatus::Pending | DeliveryStatus::RetryScheduled
    ) && row.claim.next_attempt_at <= now
        && (row.claim.claim_owner.is_none() || row.claim.claim_expires_at <= now)
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
pub fn complete_mail_ingress(
    ctx: &ReducerContext,
    ingress_id: String,
    instance_id: String,
    delivery_count: u32,
    failed_delivery_count: u32,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let mut row = require_ingress_claimed_by_me(ctx, &ingress_id, &instance_id)?;

    row.claim.status = DeliveryStatus::Completed;
    row.delivery_count = delivery_count;
    row.failed_delivery_count = failed_delivery_count;
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

    if let Some(mut existing) = ctx.db.mail_delivery_pending().id().find(&delivery_id) {
        debug_assert_eq!(existing.row.ingress_id, ingress.id);
        debug_assert_eq!(existing.row.subscription_id, subscription_id);
        debug_assert_eq!(existing.row.recipient_email, recipient_email);

        let next_attempt_at = ctx.timestamp;
        existing.row.recipient_account_id = recipient_account_id;
        existing.row.list_email = list_email;
        existing.row.list_name = list_name;
        existing.row.original_sender_email = original_sender_email;
        existing.row.from_header = from_header;
        existing.row.reply_to = reply_to;
        existing.row.raw_message = raw_message;
        existing.row.unsubscribe_token = unsubscribe_token;
        existing.row.updated_at = next_attempt_at;
        existing.row.next_attempt_at = next_attempt_at;
        existing.next_attempt_at = next_attempt_at;
        ctx.db.mail_delivery_pending().id().update(existing);
        return delivery_id;
    }

    let next_attempt_at = ctx.timestamp;
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
        next_attempt_at,
        last_error: None,
        smtp_status_code: None,
        smtp_response: None,
        finalized_at: Timestamp::UNIX_EPOCH,
        updated_at: ctx.timestamp,
    };

    ctx.db.mail_delivery_pending().insert(MailDeliveryPending {
        id: delivery_id.clone(),
        ingress_id: ingress.id.clone(),
        next_attempt_at,
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

    let now = ctx.timestamp;
    let mut candidates: Vec<MailDeliveryPending> = ctx
        .db
        .mail_delivery_pending()
        .next_attempt_at()
        .filter(..=now)
        .collect();

    candidates.sort_by(|a, b| {
        a.row
            .next_attempt_at
            .cmp(&b.row.next_attempt_at)
            .then(a.row.id.cmp(&b.row.id))
    });

    let Some(pending) = candidates.into_iter().next() else {
        return Ok(());
    };

    ctx.db.mail_delivery_pending().id().delete(&pending.row.id);
    ctx.db.mail_delivery_claimed().insert(MailDeliveryClaimed {
        id: pending.row.id.clone(),
        worker: ctx.sender(),
        instance_id,
        lease_expires_at: ctx.timestamp + delivery_lease_duration(),
        row: MailDeliveryRow {
            attempt_count: pending.row.attempt_count.saturating_add(1),
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
    instance_id: String,
    smtp_status_code: Option<u16>,
    smtp_response: String,
    error_kind: String,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }
    let claimed = require_claimed_by_me(ctx, &delivery_id, &instance_id)?;

    ctx.db.mail_delivery_claimed().id().delete(&delivery_id);

    if claimed.row.attempt_count >= MAX_DELIVERY_ATTEMPTS {
        ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
            id: 0,
            delivery_id: delivery_id.clone(),
            occurred_at: ctx.timestamp,
            event_type: DeliveryStatus::Failed.to_string(),
            attempt_no: claimed.row.attempt_count,
            smtp_status_code,
            smtp_response: Some(smtp_response.clone()),
            error_kind: Some(error_kind),
            details: format!("SMTP delivery failed (max attempts reached): {smtp_response}"),
            worker_identity: Some(ctx.sender()),
        });

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
    } else {
        let backoff = delivery_retry_backoff(claimed.row.attempt_count);
        let next_attempt_at = ctx.timestamp + backoff;

        ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
            id: 0,
            delivery_id: delivery_id.clone(),
            occurred_at: ctx.timestamp,
            event_type: DeliveryStatus::RetryScheduled.to_string(),
            attempt_no: claimed.row.attempt_count,
            smtp_status_code,
            smtp_response: Some(smtp_response.clone()),
            error_kind: Some(error_kind),
            details: format!("SMTP retry scheduled: {smtp_response}"),
            worker_identity: Some(ctx.sender()),
        });

        ctx.db.mail_delivery_pending().insert(MailDeliveryPending {
            id: delivery_id.clone(),
            ingress_id: claimed.row.ingress_id.clone(),
            next_attempt_at,
            row: MailDeliveryRow {
                smtp_status_code,
                smtp_response: Some(smtp_response.clone()),
                last_error: Some(smtp_response),
                next_attempt_at,
                updated_at: ctx.timestamp,
                ..claimed.row
            },
        });
    }

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
    let stale: Vec<MailDeliveryClaimed> = ctx
        .db
        .mail_delivery_claimed()
        .lease_expires_at()
        .filter(..=now)
        .collect();

    for claimed in stale {
        ctx.db.mail_delivery_claimed().id().delete(&claimed.id);
        let next_attempt_at = now;
        ctx.db.mail_delivery_pending().insert(MailDeliveryPending {
            id: claimed.id.clone(),
            ingress_id: claimed.row.ingress_id.clone(),
            next_attempt_at,
            row: MailDeliveryRow {
                last_error: Some("Lease expired — requeued".to_string()),
                next_attempt_at,
                updated_at: next_attempt_at,
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
