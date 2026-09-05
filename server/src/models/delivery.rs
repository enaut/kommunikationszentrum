use spacetimedb::{Identity, ScheduleAt, Timestamp};

use crate::{
    common::delivery_types::{ClaimState, DeliveryFinalState},
    reducers::{expire_stale_delivery_claims, requeue_temporary_failed_mails},
};

/// One row per inbound message fan-out job.
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

/// Canonical outbound email message payload for a specific recipient.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_message)]
pub struct MailDeliveryMessage {
    #[primary_key]
    pub delivery_id: String,
    /// FK → MailMessage.id
    #[index(btree)]
    pub mail_message_id: u64,
    /// FK → MailIngress.id
    #[index(btree)]
    pub ingress_id: String,
    pub subscription_id: u64,
    pub recipient_email: String,
    pub recipient_account_id: Option<u64>,
    /// Rewritten From header for outbound delivery
    pub from_header: String,
    pub reply_to: String,
    /// Final assembled SMTP envelope (complete RFC 5322 message bytes)
    pub raw_message: String,
}

/// Work items available for the sender worker to claim.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_pending)]
pub struct MailDeliveryPending {
    #[primary_key]
    pub delivery_id: String,
    #[index(btree)]
    pub ingress_id: String,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub last_updated: Timestamp,
}

/// Temporary retry queue for transient SMTP failures.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_temporary_failed)]
pub struct MailDeliveryTemporaryFailed {
    #[primary_key]
    pub delivery_id: String,
    #[index(btree)]
    pub ingress_id: String,
    #[index(btree)]
    pub next_attempt_at: Timestamp,
    pub attempt_count: u32,
    pub last_error: String,
    pub smtp_status_code: Option<u16>,
    pub smtp_response: Option<String>,
    pub last_updated: Timestamp,
}

/// Work items currently held by a worker (lease active).
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_claimed)]
pub struct MailDeliveryClaimed {
    #[primary_key]
    pub delivery_id: String,
    #[index(btree)]
    pub ingress_id: String,
    pub worker: Identity,
    pub instance_id: String,
    #[index(btree)]
    pub lease_expires_at: Timestamp,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub last_updated: Timestamp,
}

/// Terminal rows — sent, failed, or bounced. Immutable after insert.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_done)]
pub struct MailDeliveryDone {
    #[primary_key]
    pub delivery_id: String,
    #[index(btree)]
    pub ingress_id: String,
    pub final_state: DeliveryFinalState,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub smtp_status_code: Option<u16>,
    pub smtp_response: Option<String>,
    pub finalized_at: Timestamp,
    pub last_updated: Timestamp,
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
