use spacetimedb::Timestamp;

#[spacetimedb::table(accessor = mta_connection_log)]
pub struct MtaConnectionLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub client_ip: String,
    pub stage: String,
    pub action: String,
    pub timestamp: Timestamp,
    pub details: String,
}

#[spacetimedb::table(accessor = mta_message_log)]
pub struct MtaMessageLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub stage: String,
    pub action: String,
    pub timestamp: Timestamp,
    pub queue_id: Option<String>,
    pub topic_count: u32,
}

#[spacetimedb::table(accessor = blocked_ips)]
pub struct BlockedIp {
    #[primary_key]
    pub ip: String,
    pub reason: String,
    pub blocked_at: Timestamp,
    pub active: bool,
}

/// One row per accepted email delivery, linked to its canonical `MailMessage`
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
}
