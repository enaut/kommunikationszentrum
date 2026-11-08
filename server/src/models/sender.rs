use spacetimedb::{ScheduleAt, SpacetimeType, Timestamp};

use crate::services::sender::dispatch_jmap_mail;

#[derive(SpacetimeType, Debug, Clone, Copy, PartialEq, Eq)]
pub enum JmapDeliveryState {
    Pending,
    Importing,
    Imported,
    Submitting,
    SubmissionUnknown,
    Sent,
    Failed,
}

/// Who triggered a delivery lifecycle outcome.
#[derive(SpacetimeType, Debug, Clone, Copy, PartialEq, Eq)]
pub enum JmapDeliveryActor {
    Scheduled,
    Admin,
}

/// Durable per-recipient JMAP work. The canonical inbound content remains in `MailMessage`.
#[derive(Clone)]
#[spacetimedb::table(accessor = jmap_mail_deliveries)]
pub struct JmapMailDelivery {
    #[primary_key]
    pub delivery_id: String,
    #[index(btree)]
    pub ingress_id: Option<String>,
    #[index(btree)]
    pub topic_id: Option<u64>,
    pub subscription_id: Option<u64>,
    pub system_mail_id: Option<u64>,
    pub legacy_delivery_id: Option<String>,
    pub mail_message_id: Option<u64>,
    pub recipient_email: String,
    pub from_email: String,
    pub subject: String,
    pub body_text: String,
    pub state: JmapDeliveryState,
    #[index(btree)]
    pub next_attempt_at: Timestamp,
    pub attempt_count: u32,
    pub claim_token: Option<String>,
    pub claim_expires_at: Timestamp,
    pub jmap_email_id: Option<String>,
    pub jmap_submission_id: Option<String>,
    pub last_error: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Clone)]
#[spacetimedb::table(accessor = jmap_mail_delivery_events)]
pub struct JmapMailDeliveryEvent {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub delivery_id: String,
    #[index(btree)]
    pub occurred_at: Timestamp,
    pub event_type: String,
    pub attempt_no: u32,
    pub jmap_email_id: Option<String>,
    pub jmap_submission_id: Option<String>,
    pub details: String,
    pub actor: JmapDeliveryActor,
}

#[derive(Clone)]
#[spacetimedb::table(
    accessor = jmap_mail_dispatch_schedule,
    scheduled(dispatch_jmap_mail)
)]
pub struct JmapMailDispatchSchedule {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}
