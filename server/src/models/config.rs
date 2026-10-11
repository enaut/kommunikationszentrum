use spacetimedb::Timestamp;

#[derive(Clone)]
#[spacetimedb::table(accessor = stalwart_config)]
pub struct StalwartConfig {
    #[primary_key]
    pub id: u64,
    pub jmap_url: String,
    pub admin_token: String,
    pub updated_at: Timestamp,
}

#[derive(Clone, Debug, PartialEq)]
#[spacetimedb::table(accessor = mail_retention_config)]
pub struct MailRetentionConfig {
    #[primary_key]
    pub id: u64,
    /// Retention period for rejected messages in days (default: 30)
    pub rejected_retention_days: u32,
    pub updated_at: Timestamp,
}

