use spacetimedb::{Identity, Timestamp};

// Configuration constants that can be set at compile time via environment variables
pub const DJANGO_OAUTH_BASE_URL: &str = match option_env!("DJANGO_BASE_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:8000",
};

pub const DJANGO_OAUTH_ISSUER_PATH: &str = "/o";

#[derive(spacetimedb::SpacetimeType, Debug, Clone, PartialEq)]
pub enum EmailSource {
    DjangoSync,
    Native,
}

#[derive(Debug, Clone)]
#[spacetimedb::table(accessor = account_emails)]
pub struct AccountEmail {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub account_id: u64,
    #[index(btree)]
    pub email: String,
    pub source: EmailSource,
    pub is_verified: bool,
    #[index(btree)]
    pub added_at: Timestamp,
}

#[derive(Debug, Clone)]
#[spacetimedb::table(accessor = account)]
pub struct Account {
    #[primary_key]
    pub id: u64, // mitgliedsnr from Django
    #[unique]
    pub identity: Identity,
    pub name: String,
    #[index(btree)]
    pub primary_email_id: u64,
    pub is_active: bool,
    #[index(btree)]
    pub last_synced: Timestamp,
}

#[derive(Debug, Clone)]
#[spacetimedb::table(accessor = account_configs, public)]
pub struct AccountConfig {
    #[primary_key]
    pub account_id: u64,

    pub message_offset: u32,
    pub message_limit: u32,
    pub selected_message_category: Option<u64>,

    pub member_offset: u32,
    pub member_limit: u32,
    pub member_search_query: Option<String>,
    
    pub viewing_category_id: Option<u64>,

    pub language: Option<String>,
    pub theme: Option<String>,

    pub search_matching_accounts: u32,
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

#[derive(Debug, Clone)]
#[spacetimedb::table(accessor = email_verification_tokens)]
pub struct EmailVerificationToken {
    #[primary_key]
    pub token: String,
    pub account_id: u64,
    pub email: String,
    #[index(btree)]
    pub created_at: Timestamp,
    #[index(btree)]
    pub expires_at: Timestamp,
}
