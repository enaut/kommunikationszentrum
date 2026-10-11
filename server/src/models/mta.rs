use spacetimedb::{ScheduleAt, SpacetimeType, Timestamp};

use crate::reducers::purge_rejected_mails;

#[spacetimedb::table(accessor = blocked_ips)]
pub struct BlockedIp {
    #[primary_key]
    pub ip: String,
    pub reason: String,
    pub blocked_at: Timestamp,
    pub active: bool,
}

/// Reason why an email could not be delivered to a message topic.
#[derive(SpacetimeType, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopicRejectionReason {
    /// The user is subscribed to the topic, but only has Read permission (no Write permission).
    NoWritePermission,
    /// The user has an active account, but has no active subscription to this topic.
    NotSubscribed,
    /// The sender's email address is not associated with any active verified account in the system.
    NotRegistered,
    /// The message exceeds the mailing-list size limit.
    Oversize,
}

/// Per-topic admission outcome stored on [`ReceivedMessage`].
#[derive(SpacetimeType, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageDisposition {
    #[default]
    Accepted,
    Rejected(TopicRejectionReason),
}

/// One row per topic delivery attempt, linked to its canonical `MailMessage`
/// and the target mailing-list topic.
#[derive(Clone)]
#[spacetimedb::table(accessor = received_message)]
pub struct ReceivedMessage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    /// FK → MailMessage.id
    #[index(btree)]
    pub mail_message_id: u64,
    /// FK → MessageTopic.id (used for per-topic lookup in user view)
    #[index(btree)]
    pub topic_id: u64,
    /// The mailing-list address this message was delivered to
    pub topic_email: String,
    /// Copy of MailMessage.received_at for efficient range scans in the admin view
    /// without requiring a join.
    #[index(btree)]
    pub received_at: Timestamp,
    #[default(MessageDisposition::Accepted)]
    pub disposition: MessageDisposition,
}

/// Schedule table for the `purge_rejected_mails` reducer.
#[derive(Clone)]
#[spacetimedb::table(
    accessor = purge_rejected_mails_schedule,
    public,
    scheduled(purge_rejected_mails)
)]
pub struct PurgeRejectedMailsSchedule {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}

