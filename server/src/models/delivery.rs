use spacetimedb::{Identity, Timestamp};

use crate::common::delivery_types::{ClaimState, DeliveryFinalState};

/// Accepted inbound message state consumed by the native JMAP dispatcher.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_ingress)]
pub struct MailIngress {
    #[primary_key]
    pub id: String,
    /// FK → MailMessage.id
    #[index(btree)]
    pub mail_message_id: u64,
    #[index(btree)]
    pub topic_id: u64,
    pub topic_email: String,
    pub claim: ClaimState,
    pub recipient_count: u32,
    pub delivery_count: u32,
    pub failed_delivery_count: u32,
    pub completed_at: Timestamp,
}

/// Legacy per-recipient SMTP payload retained for pending-delivery migration.
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

/// Legacy pending SMTP work retained for migration to the JMAP queue.
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

/// Legacy SMTP retry rows retained for migration to the JMAP queue.
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

/// Legacy claimed SMTP work retained for delivery-outcome reconciliation.
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

/// Legacy terminal SMTP delivery records retained as historical state.
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

#[derive(Debug, Clone)]
#[spacetimedb::table(accessor = system_mail_pending)]
pub struct SystemMailPending {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub recipient: String,
    pub subject: String,
    pub body_text: String,
    pub instance_id: Option<String>,
    pub claimed_at: Option<Timestamp>,
}
