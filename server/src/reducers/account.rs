use serde::{Deserialize, Serialize};
use spacetimedb::{Identity, ReducerContext, Table};

use crate::common::auth::{is_admin_identity, is_admin_user};
use crate::models::account::*;

#[derive(Serialize, Deserialize)]
pub struct UserSyncData {
    pub mitgliedsnr: u64,
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<bool>,
    pub is_admin: Option<bool>,
    pub updated_at: Option<String>,
    pub identity_hex: Option<String>,
    pub categories: Option<Vec<crate::models::category::CategorySyncData>>,
    pub unsubscribe_category_emails: Option<Vec<String>>,
}

/// Add an identity to admin_identities. Only existing admins may call this.
#[spacetimedb::reducer]
pub fn register_admin_identity(ctx: &ReducerContext, identity_hex: String) -> Result<(), String> {
    log::info!("Adding admin Identity");
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
        log::info!("Identity was already listed!");
        return Ok(());
    }
    ctx.db.admin_identities().insert(AdminIdentity { identity });
    log::info!("Registered admin identity: {:?}", identity);
    Ok(())
}

/// Remove an identity from admin_identities. Only existing admins may call this.
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

#[spacetimedb::reducer]
pub fn create_webhook_token(
    ctx: &ReducerContext,
    token_hash: String,
    label: String,
    permissions: Vec<String>,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: only admins can create webhook tokens".into());
    }
    if ctx
        .db
        .webhook_tokens()
        .token_hash()
        .find(&token_hash)
        .is_some()
    {
        return Err("Token already exists".into());
    }
    ctx.db.webhook_tokens().insert(WebhookToken {
        id: 0,
        token_hash: token_hash.clone(),
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
                let subscriber_email = data.email.clone().unwrap_or_default();

                if let Some(existing) = ctx.db.account().id().find(&data.mitgliedsnr) {
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

                for category in data.categories.unwrap_or_default() {
                    let category_email = category.email_address.clone();
                    if let Err(e) = crate::reducers::categories::do_add_and_subscribe_category(
                        ctx,
                        data.mitgliedsnr,
                        subscriber_email.clone(),
                        category.name,
                        category.email_address,
                        category.description,
                        category.visibility,
                        category.topics,
                        category.required,
                    ) {
                        log::error!(
                            "Failed to add/subscribe category '{}' for account {}: {}",
                            category_email,
                            data.mitgliedsnr,
                            e
                        );
                    }
                }

                for category_email in data.unsubscribe_category_emails.unwrap_or_default() {
                    if let Err(e) =
                        crate::reducers::categories::do_remove_subscription_for_category_email(
                            ctx,
                            data.mitgliedsnr,
                            &category_email,
                        )
                    {
                        log::error!(
                            "Failed to remove subscription to category '{}' for account {}: {}",
                            category_email,
                            data.mitgliedsnr,
                            e
                        );
                    }
                }
            }
            "delete" => {
                if let Some(existing) = ctx.db.account().id().find(&data.mitgliedsnr) {
                    let identity_of_user = existing.identity;
                    ctx.db.account().delete(existing);
                    log::info!("Deleted user: {} ({})", data.mitgliedsnr, action);
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
