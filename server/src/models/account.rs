use spacetimedb::{Identity, Timestamp};

// Configuration constants that can be set at compile time via environment variables
pub const DJANGO_OAUTH_BASE_URL: &str = match option_env!("DJANGO_BASE_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:8000",
};

pub const DJANGO_OAUTH_ISSUER_PATH: &str = "/o";

#[derive(Debug, Clone)]
#[spacetimedb::table(accessor = account)]
pub struct Account {
    #[primary_key]
    pub id: u64, // mitgliedsnr from Django
    #[unique]
    pub identity: Identity,
    pub name: String,
    #[index(btree)]
    pub email: String,
    pub is_active: bool,
    #[index(btree)]
    pub last_synced: Timestamp,
}

#[derive(Debug, Clone)]
#[spacetimedb::table(accessor = admin_identities)]
pub struct AdminIdentity {
    #[primary_key]
    pub identity: Identity,
}

#[derive(Debug, Clone)]
#[spacetimedb::table(accessor = webhook_tokens)]
pub struct WebhookToken {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub token_hash: String,
    pub label: String,
    pub permissions: Vec<String>,
    #[index(btree)]
    pub created_at: Timestamp,
    pub active: bool,
}
