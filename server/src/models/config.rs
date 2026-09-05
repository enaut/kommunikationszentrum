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
