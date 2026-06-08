use serde::{Deserialize, Serialize};
use spacetimedb::{Filter, Identity, ReducerContext, Table, Timestamp, ViewContext};

// Configuration constants that can be set at compile time via environment variables
const DJANGO_OAUTH_BASE_URL: &str = match option_env!("DJANGO_BASE_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:8000",
};

const DJANGO_OAUTH_ISSUER_PATH: &str = "/o";

#[derive(Debug)]
#[spacetimedb::table(accessor = account, public)]
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

#[spacetimedb::table(accessor = admin_identities, public)]
pub struct AdminIdentity {
    #[primary_key]
    pub identity: Identity,
}

#[derive(Serialize, Deserialize)]
pub struct UserSyncData {
    pub mitgliedsnr: u64,
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<bool>,
    pub is_admin: Option<bool>,
    pub updated_at: Option<String>,
    // Optional: precomputed Spacetime Identity as hex string (provided by Django)
    pub identity_hex: Option<String>,
}

// Webhook token table: stores hashed bearer tokens and permissions.
#[spacetimedb::table(accessor = webhook_tokens)]
pub struct WebhookToken {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub token_hash: String,
    pub label: String,
    pub permissions: Vec<String>,
    pub created_at: Timestamp,
    pub active: bool,
}

// Direct queries to `account` are restricted to the caller's own row.
#[spacetimedb::client_visibility_filter]
pub const ACCOUNT_VISIBILITY: Filter =
    Filter::Sql("SELECT * FROM account WHERE identity = :sender");

/// Returns only the caller's own account for regular users.
/// Returns all accounts for admins (identity present in admin_identities).
/// The admin UI subscribes to this view instead of the raw account table.
#[spacetimedb::view(accessor = visible_accounts, public)]
pub fn visible_accounts(ctx: &ViewContext) -> Vec<Account> {
    let sender = ctx.sender();
    let is_admin = ctx.db.admin_identities().identity().find(&sender).is_some();
    if is_admin {
        ctx.db
            .account()
            .last_synced()
            .filter(Timestamp::UNIX_EPOCH..)
            .collect()
    } else {
        ctx.db
            .account()
            .identity()
            .find(&sender)
            .into_iter()
            .collect()
    }
}

/// Check if the current user has admin permissions.
pub(crate) fn is_admin_user(ctx: &ReducerContext) -> bool {
    is_admin_identity(ctx, ctx.sender())
}

/// True if the provided identity is the module owner or listed in admin_identities.
pub(crate) fn is_admin_identity(ctx: &ReducerContext, who: Identity) -> bool {
    // Module owner is always admin
    if who == ctx.database_identity() {
        return true;
    }
    ctx.db.admin_identities().identity().find(&who).is_some()
}

/// Add an identity to admin_identities. Only existing admins may call this.
/// `identity_hex` is the 64-character hex string shown by the webhook-proxy on first start.
#[spacetimedb::reducer]
pub fn register_admin_identity(ctx: &ReducerContext, identity_hex: String) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: only admins can register admin identities".into());
    }
    let identity = Identity::from_hex(&identity_hex)
        .map_err(|e| format!("Invalid identity hex '{}': {}", identity_hex, e))?;
    if ctx
        .db
        .admin_identities()
        .identity()
        .find(&identity)
        .is_some()
    {
        return Ok(()); // idempotent
    }
    ctx.db.admin_identities().insert(AdminIdentity { identity });
    log::info!("Registered admin identity: {:?}", identity);
    Ok(())
}

/// Remove an identity from admin_identities. Only existing admins may call this.
/// `identity_hex` is the 64-character hex string of the identity to remove.
#[spacetimedb::reducer]
pub fn unregister_admin_identity(ctx: &ReducerContext, identity_hex: String) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: only admins can unregister admin identities".into());
    }
    let identity = Identity::from_hex(&identity_hex)
        .map_err(|e| format!("Invalid identity hex '{}': {}", identity_hex, e))?;
    ctx.db.admin_identities().identity().delete(&identity);
    log::info!("Unregistered admin identity: {:?}", identity);
    Ok(())
}

// New reducers for webhook token management
#[spacetimedb::reducer]
pub fn create_webhook_token(
    ctx: &ReducerContext,
    token_plain: String,
    label: String,
    permissions: Vec<String>,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: only admins can create webhook tokens".into());
    }
    let hash = hex::encode(blake3::hash(token_plain.as_bytes()).as_bytes());
    if ctx.db.webhook_tokens().token_hash().find(&hash).is_some() {
        return Err("Token already exists".into());
    }
    ctx.db.webhook_tokens().insert(WebhookToken {
        id: 0,
        token_hash: hash,
        label: label.clone(),
        permissions,
        created_at: ctx.timestamp,
        active: true,
    });
    log::info!("Created webhook token (label: {})", label);
    Ok(())
}

#[spacetimedb::reducer]
pub fn revoke_webhook_token(ctx: &ReducerContext, token_hash: String) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: only admins can revoke webhook tokens".into());
    }
    ctx.db.webhook_tokens().token_hash().delete(&token_hash);
    log::info!("Revoked webhook token: {}", token_hash);
    Ok(())
}

// Keep existing sync_user logic but factor into helper so HTTP handler can call it.

pub(crate) fn do_sync_user(
    ctx: &ReducerContext,
    action: String,
    user_data: String,
) -> Result<(), String> {
    let timestamp = ctx.timestamp;

    log::info!("Syncing user with action: {}", action);
    log::info!("User data: {}", user_data);

    match serde_json::from_str::<UserSyncData>(&user_data) {
        Ok(data) => match action.as_str() {
            "upsert" => {
                log::info!("Syncing user: {} ({})", data.mitgliedsnr, action);

                let mitgliedsnr = data.mitgliedsnr.to_string();
                let issuer_url = format!("{}{}", DJANGO_OAUTH_BASE_URL, DJANGO_OAUTH_ISSUER_PATH);
                let identity_of_user = Identity::from_claims(&issuer_url, &mitgliedsnr);
                let is_admin = data.is_admin.unwrap_or(false);

                if let Some(existing) = ctx.db.account().id().find(&data.mitgliedsnr) {
                    // Update in place — Django is source of truth for is_admin
                    let updated = Account {
                        identity: identity_of_user,
                        name: data.name.unwrap_or_default(),
                        email: data.email.unwrap_or_default(),
                        is_active: data.is_active.unwrap_or(true),
                        last_synced: timestamp,
                        ..existing
                    };
                    ctx.db.account().id().update(updated);
                    log::info!("Updated existing account: {}", data.mitgliedsnr);
                } else {
                    // Insert new account
                    let account = Account {
                        id: data.mitgliedsnr,
                        identity: identity_of_user,
                        name: data.name.unwrap_or_default(),
                        email: data.email.unwrap_or_default(),
                        is_active: data.is_active.unwrap_or(true),
                        last_synced: timestamp,
                    };
                    log::info!("Inserting new account: {:#?}", account);
                    ctx.db.account().insert(account);
                    log::info!("Inserted new account: {}", data.mitgliedsnr);
                }

                // Keep admin_identities table in sync with Django's admin flag
                if is_admin {
                    if ctx
                        .db
                        .admin_identities()
                        .identity()
                        .find(&identity_of_user)
                        .is_none()
                    {
                        ctx.db.admin_identities().insert(AdminIdentity {
                            identity: identity_of_user,
                        });
                        log::info!("Granted admin_identities for account: {}", data.mitgliedsnr);
                    }
                } else if ctx
                    .db
                    .admin_identities()
                    .identity()
                    .find(&identity_of_user)
                    .is_some()
                {
                    ctx.db
                        .admin_identities()
                        .identity()
                        .delete(&identity_of_user);
                    log::info!("Revoked admin_identities for account: {}", data.mitgliedsnr);
                }
            }
            "delete" => {
                // Find and delete the account
                if let Some(existing) = ctx.db.account().id().find(&data.mitgliedsnr) {
                    let identity_of_user = existing.identity;
                    ctx.db.account().delete(existing);
                    log::info!("Deleted user: {} ({})", data.mitgliedsnr, action);
                    // Also remove from admin_identities if present
                    if ctx
                        .db
                        .admin_identities()
                        .identity()
                        .find(&identity_of_user)
                        .is_some()
                    {
                        ctx.db
                            .admin_identities()
                            .identity()
                            .delete(&identity_of_user);
                        log::info!(
                            "Removed admin_identities for deleted account: {}",
                            data.mitgliedsnr
                        );
                    }
                }
            }
            _ => {
                return Err(format!("Unknown sync action: {}", action));
            }
        },
        Err(e) => {
            return Err(format!("Failed to parse user sync data: {}", e));
        }
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn sync_user(ctx: &ReducerContext, action: String, user_data: String) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        log::warn!("Unauthorized sync_user call from {:?}", ctx.sender());
        return Err(format!(
            "Unauthorized: sync_user called by {:?}",
            ctx.sender()
        ));
    }
    do_sync_user(ctx, action, user_data)
}
