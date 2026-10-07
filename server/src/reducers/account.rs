use serde::{Deserialize, Serialize};
use spacetimedb::{Identity, ReducerContext, Table};

use crate::common::auth::{is_admin_identity, is_admin_user};
use crate::models::account::*;
use crate::models::topic::{subscription_unsubscribe_tokens, subscriptions};

use crate::models::delivery::*;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SyncedEmail {
    pub email: String,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub is_verified: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserSyncData {
    pub external_id: String,
    pub name: Option<String>,
    pub is_active: Option<bool>,
    pub is_admin: Option<bool>,
    pub identity_hex: Option<String>,
    #[serde(default)]
    pub topics: Vec<crate::models::topic::TopicSyncData>,
    #[serde(default)]
    pub unsubscribe_topic_emails: Vec<String>,
    #[serde(default)]
    pub emails: Vec<SyncedEmail>,
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

    // Cannot remove database identity
    if identity == ctx.database_identity() {
        return Err("Cannot remove the database identity from administrators".into());
    }

    // Cannot remove the last administrator
    if ctx.db.admin_identities().count() <= 1 {
        return Err("Cannot remove the last administrator identity".into());
    }

    ctx.db.admin_identities().identity().delete(&identity);
    log::info!("Unregistered admin identity: {:?}", identity);
    Ok(())
}

/// Helper to extract string or first string element in array from json value
fn extract_string_or_first_array(val: &serde_json::Value) -> Option<String> {
    match val {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if !trimmed.is_empty() && trimmed.contains('@') {
                Some(trimmed.to_string())
            } else {
                None
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(s) = extract_string_or_first_array(item) {
                    return Some(s);
                }
            }
            None
        }
        serde_json::Value::Object(map) => {
            for key in &["value", "email", "address"] {
                if let Some(item) = map.get(*key) {
                    if let Some(s) = extract_string_or_first_array(item) {
                        return Some(s);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract email address from claims JSON, supporting standard and provider-specific keys/formats
pub fn extract_email_from_claims(claims: &serde_json::Value) -> Option<String> {
    for key in &["email", "mail", "emails", "email_address"] {
        if let Some(val) = claims.get(*key) {
            if let Some(email) = extract_string_or_first_array(val) {
                return Some(email);
            }
        }
    }
    None
}

/// Extract boolean value flexibly (accepting bool, string like "true"/"1", or integer 1)
pub fn extract_bool_from_claims(val: Option<&serde_json::Value>) -> bool {
    match val {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => {
            let s = s.trim();
            s.eq_ignore_ascii_case("true") || s == "1" || s.eq_ignore_ascii_case("yes")
        }
        Some(serde_json::Value::Number(n)) => n.as_i64() == Some(1),
        _ => false,
    }
}

fn validate_token_issuer(claims: &serde_json::Value) -> Result<(), String> {
    let issuer = claims
        .get("iss")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "JWT is missing a string issuer claim".to_string())?;

    if issuer != OIDC_ISSUER_URL {
        return Err("JWT issuer does not match the configured OIDC issuer".into());
    }

    Ok(())
}

fn ensure_account_identity_matches(
    existing_identity: &Identity,
    sender: &Identity,
) -> Result<(), String> {
    if existing_identity != sender {
        return Err("Account identity does not match the authenticated sender".into());
    }

    Ok(())
}

fn require_token_email(token_email: Option<String>) -> Result<String, String> {
    token_email.ok_or_else(|| "Email address is required in the authenticated JWT".into())
}

fn should_promote_to_primary(
    is_candidate_verified: bool,
    current_primary_is_verified: Option<bool>,
) -> bool {
    is_candidate_verified || current_primary_is_verified.map_or(true, |verified| !verified)
}

/// Register or update an account for the currently connected user.
/// Called by the web client after OIDC authentication.
#[spacetimedb::reducer]
pub fn register_self(
    ctx: &ReducerContext,
    external_id: String,
    name: String,
) -> Result<(), String> {
    let sender = ctx.sender();
    let timestamp = ctx.timestamp;

    if external_id.trim().is_empty() {
        return Err("external_id cannot be empty".into());
    }

    let jwt = ctx
        .sender_auth()
        .jwt()
        .ok_or_else(|| "register_self requires an authenticated OIDC connection".to_string())?;

    let token_sub = jwt.subject();
    if token_sub != external_id {
        return Err(format!(
            "Provided external_id '{}' does not match token subject '{}'",
            external_id, token_sub
        ));
    }

    let claims: serde_json::Value = serde_json::from_str(jwt.raw_payload())
        .map_err(|e| format!("Invalid JWT payload: {}", e))?;
    validate_token_issuer(&claims)?;

    let token_email = extract_email_from_claims(&claims);

    let is_token_email_verified = token_email.is_some()
        && (extract_bool_from_claims(claims.get("email_verified"))
            || extract_bool_from_claims(claims.get("emailVerified"))
            || extract_bool_from_claims(claims.get("verified")));

    // Check if an account already exists for this sender identity or external_id (e.g. from user-sync)
    let existing_account = ctx
        .db
        .account()
        .identity()
        .find(&sender)
        .or_else(|| ctx.db.account().external_id().find(&external_id));

    if let Some(mut existing_account) = existing_account {
        if existing_account.external_id != token_sub {
            return Err("Account external_id mismatch".into());
        }
        ensure_account_identity_matches(&existing_account.identity, &sender)?;

        let mut changed = false;
        if !name.trim().is_empty() && existing_account.name != name {
            existing_account.name = name.clone();
            changed = true;
        }

        let candidate_email = token_email;

        let current_primary = ctx
            .db
            .account_emails()
            .id()
            .find(&existing_account.primary_email_id);

        if let Some(new_email) = candidate_email {
            let email_changed = current_primary.as_ref().map(|e| &e.email) != Some(&new_email);

            if email_changed {
                let existing_email_row = ctx
                    .db
                    .account_emails()
                    .account_id()
                    .filter(&existing_account.id)
                    .find(|e| e.email == new_email);

                let current_verified = current_primary.as_ref().map(|p| p.is_verified);

                if let Some(mut email_row) = existing_email_row {
                    if is_token_email_verified && !email_row.is_verified {
                        email_row.is_verified = true;
                        ctx.db.account_emails().id().update(email_row.clone());
                        changed = true;
                    }
                    if should_promote_to_primary(email_row.is_verified, current_verified)
                        && existing_account.primary_email_id != email_row.id
                    {
                        existing_account.primary_email_id = email_row.id;
                        changed = true;
                    }
                } else {
                    let new_email_row = ctx.db.account_emails().insert(AccountEmail {
                        id: 0,
                        account_id: existing_account.id,
                        email: new_email,
                        source: EmailSource::Native,
                        is_verified: is_token_email_verified,
                        added_at: timestamp,
                    });
                    changed = true;
                    if should_promote_to_primary(is_token_email_verified, current_verified) {
                        existing_account.primary_email_id = new_email_row.id;
                    }
                }
            } else if let Some(mut primary) = current_primary {
                if is_token_email_verified && !primary.is_verified {
                    primary.is_verified = true;
                    ctx.db.account_emails().id().update(primary);
                    changed = true;
                }
            }
        }

        if changed {
            existing_account.last_synced = timestamp;
            ctx.db.account().id().update(existing_account);
        }
        return Ok(());
    }

    // New accounts require an email address from the authenticated JWT.
    let reg_email = require_token_email(token_email)?;

    let primary_email = ctx.db.account_emails().insert(AccountEmail {
        id: 0,
        account_id: 0,
        email: reg_email,
        source: EmailSource::Native,
        is_verified: is_token_email_verified,
        added_at: timestamp,
    });

    let new_account = ctx.db.account().insert(Account {
        id: 0,
        external_id: external_id.clone(),
        identity: sender,
        name,
        primary_email_id: primary_email.id,
        is_active: true,
        last_synced: timestamp,
    });

    // Fix up primary email's account_id
    let mut email_row = primary_email;
    email_row.account_id = new_account.id;
    ctx.db.account_emails().id().update(email_row);

    // Ensure account config exists
    if ctx
        .db
        .account_configs()
        .account_id()
        .find(&new_account.id)
        .is_none()
    {
        ctx.db.account_configs().insert(AccountConfig {
            account_id: new_account.id,
            message_offset: 0,
            message_limit: 50,
            selected_message_topic: None,
            member_offset: 0,
            member_limit: 50,
            member_search_query: None,
            viewing_topic_id: None,
            language: None,
            theme: None,
            search_matching_accounts: 0,
        });
    }

    log::info!(
        "Registered self for external_id: {} (account_id: {})",
        external_id,
        new_account.id
    );
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
                log::info!("Syncing user: {} ({})", data.external_id, action);

                let primary_sync = data.emails.iter().find(|e| e.is_primary).ok_or_else(|| {
                    "User sync upsert requires at least one email marked with is_primary = true"
                        .to_string()
                })?;

                let issuer_url = OIDC_ISSUER_URL;
                let identity_of_user = Identity::from_claims(issuer_url, &data.external_id);

                // Look up existing account by external_id
                let existing_account = ctx.db.account().external_id().find(&data.external_id);
                let account_id = existing_account.as_ref().map(|a| a.id).unwrap_or(0);

                // Synchronize all emails from data.emails
                let mut primary_email_id = 0;
                let mut new_email_ids = Vec::new();

                for synced in &data.emails {
                    let email_id = if let Some(mut existing) = ctx
                        .db
                        .account_emails()
                        .account_id()
                        .filter(&account_id)
                        .find(|e| e.email == synced.email)
                    {
                        let mut changed = false;
                        if existing.source != EmailSource::ExternalSync {
                            existing.source = EmailSource::ExternalSync;
                            changed = true;
                        }
                        if synced.is_verified && !existing.is_verified {
                            existing.is_verified = true;
                            changed = true;
                        }
                        if changed {
                            ctx.db.account_emails().id().update(existing.clone());
                        }
                        existing.id
                    } else {
                        let new_email = ctx.db.account_emails().insert(AccountEmail {
                            id: 0,
                            account_id,
                            email: synced.email.clone(),
                            source: EmailSource::ExternalSync,
                            is_verified: synced.is_verified,
                            added_at: timestamp,
                        });
                        new_email_ids.push(new_email.id);
                        new_email.id
                    };

                    if synced.email == primary_sync.email {
                        primary_email_id = email_id;
                    }
                }

                let final_account_id = if let Some(existing) = existing_account {
                    let updated = Account {
                        identity: identity_of_user,
                        name: data.name.unwrap_or_default(),
                        primary_email_id,
                        is_active: data.is_active.unwrap_or(true),
                        last_synced: timestamp,
                        ..existing
                    };
                    ctx.db.account().id().update(updated);
                    log::info!("Updated existing account: {}", data.external_id);
                    account_id
                } else {
                    let account = Account {
                        id: 0, // auto_inc
                        external_id: data.external_id.clone(),
                        identity: identity_of_user,
                        name: data.name.unwrap_or_default(),
                        primary_email_id,
                        is_active: data.is_active.unwrap_or(true),
                        last_synced: timestamp,
                    };
                    log::info!("Inserting new account: {:#?}", account);
                    let inserted = ctx.db.account().insert(account);
                    let new_id = inserted.id;
                    log::info!("Inserted new account: {} (id={})", data.external_id, new_id);

                    // Fix up the newly created emails' account_id to point to the newly assigned auto-inc id
                    for email_id in new_email_ids {
                        if let Some(mut email_row) = ctx.db.account_emails().id().find(&email_id) {
                            email_row.account_id = new_id;
                            ctx.db.account_emails().id().update(email_row);
                        }
                    }

                    new_id
                };

                if ctx
                    .db
                    .account_configs()
                    .account_id()
                    .find(&final_account_id)
                    .is_none()
                {
                    ctx.db
                        .account_configs()
                        .insert(crate::models::account::AccountConfig {
                            account_id: final_account_id,
                            message_offset: 0,
                            message_limit: 50,
                            selected_message_topic: None,
                            member_offset: 0,
                            member_limit: 50,
                            member_search_query: None,
                            viewing_topic_id: None,
                            language: None,
                            theme: None,
                            search_matching_accounts: 0,
                        });
                }

                match data.is_admin {
                    Some(true) => {
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
                            log::info!("Granted admin_identities for account: {}", data.external_id);
                        }
                    }
                    Some(false) => {
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
                            log::info!("Revoked admin_identities for account: {}", data.external_id);
                        }
                    }
                    None => {
                        // When is_admin is omitted, do not alter existing admin status
                    }
                }

                // 1. Remove ExternalSync emails not in incoming payload
                let incoming_emails: Vec<&str> =
                    data.emails.iter().map(|e| e.email.as_str()).collect();
                let existing_emails: Vec<_> = ctx
                    .db
                    .account_emails()
                    .account_id()
                    .filter(&final_account_id)
                    .collect();
                for existing in existing_emails {
                    if existing.source == EmailSource::ExternalSync
                        && !incoming_emails.contains(&existing.email.as_str())
                    {
                        // Migrate or delete subscriptions associated with this removed email
                        let orphan_subs: Vec<_> = ctx
                            .db
                            .subscriptions()
                            .account_email_id()
                            .filter(&existing.id)
                            .collect();

                        for mut sub in orphan_subs {
                            let already_subbed_on_primary = ctx
                                .db
                                .subscriptions()
                                .subscriber_account_id()
                                .filter(&final_account_id)
                                .any(|s| {
                                    s.topic_id == sub.topic_id
                                        && s.account_email_id == primary_email_id
                                });

                            if already_subbed_on_primary {
                                if let Some(tok) = ctx
                                    .db
                                    .subscription_unsubscribe_tokens()
                                    .subscription_id()
                                    .find(&sub.id)
                                {
                                    ctx.db
                                        .subscription_unsubscribe_tokens()
                                        .token()
                                        .delete(&tok.token);
                                }
                                ctx.db.subscriptions().id().delete(&sub.id);
                                log::info!(
                                    "Removed duplicate subscription {} for account {} after email {} removed",
                                    sub.id,
                                    data.external_id,
                                    existing.email
                                );
                            } else {
                                sub.account_email_id = primary_email_id;
                                ctx.db.subscriptions().id().update(sub.clone());
                                log::info!(
                                    "Migrated subscription {} to primary_email_id {} for account {} after email {} removed",
                                    sub.id,
                                    primary_email_id,
                                    data.external_id,
                                    existing.email
                                );
                            }
                        }

                        ctx.db.account_emails().id().delete(&existing.id);
                        log::info!("Removed old ExternalSync email: {}", existing.email);
                    }
                }

                for topic in data.topics {
                    let topic_email = topic.email_address.clone();
                    if let Err(e) = crate::reducers::topics::do_add_and_subscribe_topic(
                        ctx,
                        final_account_id,
                        primary_email_id,
                        topic.name,
                        topic.email_address,
                        topic.description,
                        topic.visibility,
                        topic.categories,
                        topic.required,
                        topic.default_permission,
                    ) {
                        log::error!(
                            "Failed to add/subscribe topic '{}' for account {}: {}",
                            topic_email,
                            data.external_id,
                            e
                        );
                    }
                }

                for topic_email in data.unsubscribe_topic_emails {
                    if let Err(e) = crate::reducers::topics::do_remove_subscription_for_topic_email(
                        ctx,
                        final_account_id,
                        &topic_email,
                    ) {
                        log::error!(
                            "Failed to remove subscription to topic '{}' for account {}: {}",
                            topic_email,
                            data.external_id,
                            e
                        );
                    }
                }
            }
            "delete" => {
                if let Some(existing) = ctx.db.account().external_id().find(&data.external_id) {
                    let account_id = existing.id;
                    let identity_of_user = existing.identity;
                    ctx.db.account().delete(existing);
                    log::info!("Deleted user: {} ({})", data.external_id, action);
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
                            data.external_id
                        );
                    }

                    // Cascade delete all subscriptions and their unsubscribe tokens
                    let subs: Vec<_> = ctx
                        .db
                        .subscriptions()
                        .subscriber_account_id()
                        .filter(&account_id)
                        .collect();
                    for sub in subs {
                        if let Some(tok) = ctx
                            .db
                            .subscription_unsubscribe_tokens()
                            .subscription_id()
                            .find(&sub.id)
                        {
                            ctx.db
                                .subscription_unsubscribe_tokens()
                                .token()
                                .delete(&tok.token);
                        }
                        ctx.db.subscriptions().id().delete(&sub.id);
                    }
                    log::info!(
                        "Removed subscriptions for deleted account: {}",
                        data.external_id
                    );

                    // Cascade delete all linked emails
                    let emails: Vec<_> = ctx
                        .db
                        .account_emails()
                        .account_id()
                        .filter(&account_id)
                        .collect();
                    for email in emails {
                        ctx.db.account_emails().id().delete(&email.id);
                    }
                    log::info!(
                        "Removed account emails for deleted account: {}",
                        data.external_id
                    );

                    // Cascade delete pending verification tokens
                    let tokens: Vec<_> = ctx
                        .db
                        .email_verification_tokens()
                        .iter()
                        .filter(|t| t.account_id == account_id)
                        .collect();
                    for t in tokens {
                        ctx.db.email_verification_tokens().token().delete(&t.token);
                    }
                    log::info!(
                        "Removed email verification tokens for deleted account: {}",
                        data.external_id
                    );
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

#[spacetimedb::reducer]
pub fn admin_add_account_email(
    ctx: &ReducerContext,
    account_id: u64,
    email: String,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized".into());
    }

    if ctx
        .db
        .account_emails()
        .account_id()
        .filter(&account_id)
        .any(|e| e.email == email)
    {
        return Err("Email already registered for this account".into());
    }

    ctx.db.account_emails().insert(AccountEmail {
        id: 0,
        account_id,
        email,
        source: EmailSource::Native,
        is_verified: true, // Admins bypass verification
        added_at: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn user_request_email_verification(ctx: &ReducerContext, email: String) -> Result<(), String> {
    let account = ctx
        .db
        .account()
        .identity()
        .find(&ctx.sender())
        .ok_or_else(|| "Account not found for sender".to_string())?;

    let existing_email = ctx
        .db
        .account_emails()
        .account_id()
        .filter(&account.id)
        .find(|e| e.email == email);

    if let Some(existing) = existing_email {
        if existing.is_verified {
            return Err("Email already registered for this account".into());
        }
        // Email exists but is unverified: clean up previous tokens for this email before issuing a new one
        let old_tokens: Vec<_> = ctx
            .db
            .email_verification_tokens()
            .iter()
            .filter(|t| t.account_id == account.id && t.email == email)
            .collect();
        for t in old_tokens {
            ctx.db.email_verification_tokens().token().delete(&t.token);
        }
    } else {
        // Insert unverified email so it immediately appears in the member's list
        ctx.db.account_emails().insert(AccountEmail {
            id: 0,
            account_id: account.id,
            email: email.clone(),
            source: EmailSource::Native,
            is_verified: false,
            added_at: ctx.timestamp,
        });
    }

    // Generate token
    let hash = spacetimedb::spacetimedb_lib::hash::hash_bytes(
        format!("{:?}:{}:{}", ctx.timestamp, account.id, email).as_bytes(),
    );
    let token = hex::encode(hash.data.as_slice());

    // Create verification token (expires in 24h)
    ctx.db
        .email_verification_tokens()
        .insert(EmailVerificationToken {
            token: token.clone(),
            account_id: account.id,
            email: email.clone(),
            created_at: ctx.timestamp,
            expires_at: ctx.timestamp + spacetimedb::TimeDuration::from_micros(86400 * 1_000_000),
        });

    // Queue system email to the newly added address only
    let subject = "Verify your email for Kommunikationszentrum".to_string();
    let base_url = option_env!("FRONTEND_BASE_URL").unwrap_or(FRONTEND_BASE_URL);
    let link = format!("{}/?token={}", base_url.trim_end_matches('/'), token);
    let body_text = format!(
        "Please verify your email address by clicking the following link:\n\n{}",
        link
    );

    ctx.db.system_mail_pending().insert(SystemMailPending {
        id: 0,
        recipient: email,
        subject,
        body_text,
        instance_id: None,
        claimed_at: None,
    });

    Ok(())
}

#[spacetimedb::reducer]
pub fn user_verify_email(ctx: &ReducerContext, token: String) -> Result<(), String> {
    let verification = ctx
        .db
        .email_verification_tokens()
        .token()
        .find(&token)
        .ok_or_else(|| "Invalid or expired token".to_string())?;

    if ctx.timestamp > verification.expires_at {
        ctx.db.email_verification_tokens().token().delete(&token);
        return Err("Token expired".into());
    }

    // Mark existing unverified email as verified, or insert if not present
    if let Some(mut existing) = ctx
        .db
        .account_emails()
        .account_id()
        .filter(&verification.account_id)
        .find(|e| e.email == verification.email)
    {
        existing.is_verified = true;
        ctx.db.account_emails().id().update(existing);
    } else {
        ctx.db.account_emails().insert(AccountEmail {
            id: 0,
            account_id: verification.account_id,
            email: verification.email.clone(),
            source: EmailSource::Native,
            is_verified: true,
            added_at: ctx.timestamp,
        });
    }

    // Delete token
    ctx.db.email_verification_tokens().token().delete(&token);

    Ok(())
}

#[spacetimedb::reducer]
pub fn remove_account_email(ctx: &ReducerContext, account_email_id: u64) -> Result<(), String> {
    let email_row = ctx
        .db
        .account_emails()
        .id()
        .find(&account_email_id)
        .ok_or("Not found")?;

    if email_row.source == EmailSource::ExternalSync {
        return Err("Cannot remove emails synced from an external system. Please remove it in your external profile settings.".into());
    }

    let is_admin = is_admin_user(ctx);
    let is_self = ctx
        .db
        .account()
        .id()
        .find(&email_row.account_id)
        .map(|a| a.identity == ctx.sender())
        .unwrap_or(false);

    if !is_admin && !is_self {
        return Err("Unauthorized".into());
    }

    // Don't allow removing primary email
    if let Some(acc) = ctx.db.account().id().find(&email_row.account_id) {
        if acc.primary_email_id == account_email_id {
            return Err("Cannot remove primary email".into());
        }
    }

    // Remove subscriptions associated with this email and clean up tokens
    let subs: Vec<_> = ctx
        .db
        .subscriptions()
        .account_email_id()
        .filter(&account_email_id)
        .collect();
    for sub in subs {
        if let Some(tok) = ctx
            .db
            .subscription_unsubscribe_tokens()
            .subscription_id()
            .find(&sub.id)
        {
            ctx.db
                .subscription_unsubscribe_tokens()
                .token()
                .delete(&tok.token);
        }
        ctx.db.subscriptions().id().delete(&sub.id);
    }

    // Also remove any pending verification tokens for this email & account
    let tokens: Vec<_> = ctx
        .db
        .email_verification_tokens()
        .iter()
        .filter(|t| t.account_id == email_row.account_id && t.email == email_row.email)
        .collect();
    for t in tokens {
        ctx.db.email_verification_tokens().token().delete(&t.token);
    }

    ctx.db.account_emails().id().delete(&account_email_id);
    Ok(())
}

#[spacetimedb::reducer]
pub fn claim_system_mail(
    ctx: &ReducerContext,
    mail_id: u64,
    instance_id: String,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized".into());
    }
    let mut mail = ctx
        .db
        .system_mail_pending()
        .id()
        .find(&mail_id)
        .ok_or_else(|| format!("System mail {} not found", mail_id))?;

    let lease_duration = spacetimedb::TimeDuration::from_micros(300 * 1_000_000);
    if let (Some(claimed_instance), Some(claimed_time)) = (&mail.instance_id, mail.claimed_at) {
        if ctx.timestamp < claimed_time + lease_duration && claimed_instance != &instance_id {
            return Err(format!(
                "System mail {} is already claimed by instance {}",
                mail_id, claimed_instance
            ));
        }
    }

    mail.instance_id = Some(instance_id);
    mail.claimed_at = Some(ctx.timestamp);
    ctx.db.system_mail_pending().id().update(mail);
    Ok(())
}

#[spacetimedb::reducer]
pub fn release_system_mail(ctx: &ReducerContext, mail_id: u64) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized".into());
    }
    if let Some(mut mail) = ctx.db.system_mail_pending().id().find(&mail_id) {
        mail.instance_id = None;
        mail.claimed_at = None;
        ctx.db.system_mail_pending().id().update(mail);
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn complete_system_mail(ctx: &ReducerContext, mail_id: u64) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized".into());
    }
    ctx.db.system_mail_pending().id().delete(&mail_id);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_account_config(
    ctx: &ReducerContext,
    message_offset: Option<u32>,
    message_limit: Option<u32>,
    selected_message_topic: Option<u64>,
    clear_selected_message_topic: bool,
    member_offset: Option<u32>,
    member_limit: Option<u32>,
    member_search_query: Option<String>,
    clear_member_search_query: bool,
    viewing_topic_id: Option<u64>,
    clear_viewing_topic_id: bool,
    language: Option<String>,
    theme: Option<String>,
) -> Result<(), String> {
    let sender = ctx.sender();
    let account = ctx
        .db
        .account()
        .identity()
        .find(&sender)
        .ok_or_else(|| "Account not found for sender".to_string())?;

    let mut config = ctx
        .db
        .account_configs()
        .account_id()
        .find(&account.id)
        .unwrap_or_else(|| AccountConfig {
            account_id: account.id,
            message_offset: 0,
            message_limit: 50,
            selected_message_topic: None,
            member_offset: 0,
            member_limit: 50,
            member_search_query: None,
            viewing_topic_id: None,
            language: None,
            theme: None,
            search_matching_accounts: 0,
        });

    if let Some(mo) = message_offset {
        config.message_offset = mo;
    }
    if let Some(ml) = message_limit {
        config.message_limit = ml;
    }
    if clear_selected_message_topic {
        config.selected_message_topic = None;
    } else if let Some(smt) = selected_message_topic {
        config.selected_message_topic = Some(smt);
    }

    if let Some(mo) = member_offset {
        config.member_offset = mo;
    }
    if let Some(ml) = member_limit {
        config.member_limit = ml;
    }
    if clear_member_search_query {
        config.member_search_query = None;
    } else if let Some(msq) = member_search_query {
        config.member_search_query = Some(msq);
    }

    if clear_viewing_topic_id {
        config.viewing_topic_id = None;
    } else if let Some(vtid) = viewing_topic_id {
        config.viewing_topic_id = Some(vtid);
    }

    if let Some(val) = language {
        config.language = Some(val);
    }
    if let Some(val) = theme {
        config.theme = Some(val);
    }

    // Update search matching accounts metric for the user
    config.search_matching_accounts = if let Some(query) = &config.member_search_query {
        let q = query.to_lowercase();
        ctx.db
            .account()
            .iter()
            .filter(|acc| {
                account_matches_search_query(acc, &q, || {
                    ctx.db
                        .account_emails()
                        .account_id()
                        .filter(&acc.id)
                        .any(|e| e.email.to_lowercase().contains(&q))
                })
            })
            .count() as u32
    } else {
        ctx.db.account().count() as u32
    };

    if ctx
        .db
        .account_configs()
        .account_id()
        .find(&account.id)
        .is_some()
    {
        ctx.db.account_configs().account_id().update(config);
    } else {
        ctx.db.account_configs().insert(config);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_sync_data_payload_shape() {
        let payload = r#"{
            "external_id": "43",
            "name": "Max Mustermann",
            "emails": [
                {
                    "email": "max@example.org",
                    "is_primary": true,
                    "is_verified": true
                },
                {
                    "email": "alt@example.org",
                    "is_primary": false,
                    "is_verified": false
                }
            ],
            "topics": [
                {
                    "name": "VP Süd",
                    "email_address": "vp-sued@solawi.org",
                    "description": "Verteilpunkt Süd",
                    "categories": ["Verteilpunkt"]
                }
            ],
            "unsubscribe_topic_emails": ["vp-nord@solawi.org"]
        }"#;

        let data: UserSyncData = serde_json::from_str(payload).unwrap();
        assert_eq!(data.external_id, "43");
        assert_eq!(data.emails.len(), 2);
        assert_eq!(data.emails[0].email, "max@example.org");
        assert!(data.emails[0].is_primary);
        assert!(data.emails[0].is_verified);
        assert_eq!(data.emails[1].email, "alt@example.org");
        assert!(!data.emails[1].is_primary);
        assert!(!data.emails[1].is_verified);
        assert_eq!(data.topics.len(), 1);
        assert_eq!(data.topics[0].name, "VP Süd");
        assert_eq!(
            data.topics[0].categories,
            Some(vec!["Verteilpunkt".to_string()])
        );
        assert_eq!(
            data.unsubscribe_topic_emails,
            vec!["vp-nord@solawi.org".to_string()]
        );
        assert_eq!(data.is_admin, None);
    }

    #[test]
    fn test_user_sync_data_is_admin_variations() {
        let payload_none = r#"{"external_id": "1", "emails": []}"#;
        let data_none: UserSyncData = serde_json::from_str(payload_none).unwrap();
        assert_eq!(data_none.is_admin, None);

        let payload_true = r#"{"external_id": "1", "is_admin": true, "emails": []}"#;
        let data_true: UserSyncData = serde_json::from_str(payload_true).unwrap();
        assert_eq!(data_true.is_admin, Some(true));

        let payload_false = r#"{"external_id": "1", "is_admin": false, "emails": []}"#;
        let data_false: UserSyncData = serde_json::from_str(payload_false).unwrap();
        assert_eq!(data_false.is_admin, Some(false));
    }

    #[test]
    fn test_user_sync_data_rejects_legacy_fields() {
        let payload_mitgliedsnr = r#"{
            "mitgliedsnr": 43,
            "name": "Max Mustermann"
        }"#;
        assert!(serde_json::from_str::<UserSyncData>(payload_mitgliedsnr).is_err());

        let payload_legacy_email = r#"{
            "external_id": "43",
            "email": "legacy@example.org"
        }"#;
        assert!(serde_json::from_str::<UserSyncData>(payload_legacy_email).is_err());

        let payload_legacy_account_emails = r#"{
            "external_id": "43",
            "account_emails": ["legacy@example.org"]
        }"#;
        assert!(serde_json::from_str::<UserSyncData>(payload_legacy_account_emails).is_err());

        let payload_categories = r#"{
            "external_id": "43",
            "categories": []
        }"#;
        assert!(serde_json::from_str::<UserSyncData>(payload_categories).is_err());

        let payload_unsub = r#"{
            "external_id": "43",
            "unsubscribe_category_emails": ["vp@example.com"]
        }"#;
        assert!(serde_json::from_str::<UserSyncData>(payload_unsub).is_err());
    }

    #[test]
    fn test_extract_email_from_claims_various_formats() {
        // Standard "email" string
        let val1 = serde_json::json!({ "email": "test@example.com" });
        assert_eq!(
            extract_email_from_claims(&val1),
            Some("test@example.com".to_string())
        );

        // Nextcloud / LDAP "mail" string
        let val2 = serde_json::json!({ "mail": "ldap@example.com" });
        assert_eq!(
            extract_email_from_claims(&val2),
            Some("ldap@example.com".to_string())
        );

        // Nextcloud / LDAP array of strings
        let val3 = serde_json::json!({ "mail": ["multi@example.com", "alt@example.com"] });
        assert_eq!(
            extract_email_from_claims(&val3),
            Some("multi@example.com".to_string())
        );

        // Array in "email"
        let val4 = serde_json::json!({ "email": ["arr@example.com"] });
        assert_eq!(
            extract_email_from_claims(&val4),
            Some("arr@example.com".to_string())
        );

        // Array in "emails"
        let val5 = serde_json::json!({ "emails": ["emails@example.com"] });
        assert_eq!(
            extract_email_from_claims(&val5),
            Some("emails@example.com".to_string())
        );

        // An email-like username is not an email claim, even when email_verified is true.
        let val6 = serde_json::json!({
            "preferred_username": "upn@example.com",
            "email_verified": true
        });
        assert_eq!(extract_email_from_claims(&val6), None);

        let val7 = serde_json::json!({ "upn": "upn@example.com" });
        assert_eq!(extract_email_from_claims(&val7), None);
    }

    #[test]
    fn register_self_accepts_only_the_configured_issuer() {
        let expected_issuer = serde_json::json!({
            "iss": OIDC_ISSUER_URL,
            "sub": "42"
        });
        assert!(validate_token_issuer(&expected_issuer).is_ok());

        let other_issuer_same_subject = serde_json::json!({
            "iss": "https://other-issuer.example",
            "sub": "42"
        });
        assert!(validate_token_issuer(&other_issuer_same_subject).is_err());
        assert!(validate_token_issuer(&serde_json::json!({ "sub": "42" })).is_err());
        assert!(validate_token_issuer(&serde_json::json!({ "iss": 42 })).is_err());
    }

    #[test]
    fn new_account_requires_email_from_authenticated_jwt() {
        assert!(require_token_email(None).is_err());
        assert_eq!(
            require_token_email(Some("claimed@example.com".to_string())).unwrap(),
            "claimed@example.com"
        );
    }

    #[test]
    fn existing_account_identity_must_match_sender() {
        let existing_identity = Identity::from_claims(OIDC_ISSUER_URL, "42");
        let authenticated_identity = Identity::from_claims(OIDC_ISSUER_URL, "42");
        let same_subject_from_other_issuer =
            Identity::from_claims("https://other-issuer.example", "42");

        assert!(
            ensure_account_identity_matches(&existing_identity, &authenticated_identity).is_ok()
        );
        assert!(
            ensure_account_identity_matches(&existing_identity, &same_subject_from_other_issuer)
                .is_err()
        );
    }

    #[test]
    fn test_extract_bool_from_claims() {
        assert!(extract_bool_from_claims(Some(&serde_json::json!(true))));
        assert!(!extract_bool_from_claims(Some(&serde_json::json!(false))));
        assert!(extract_bool_from_claims(Some(&serde_json::json!("true"))));
        assert!(extract_bool_from_claims(Some(&serde_json::json!("True"))));
        assert!(extract_bool_from_claims(Some(&serde_json::json!("TRUE"))));
        assert!(extract_bool_from_claims(Some(&serde_json::json!("1"))));
        assert!(extract_bool_from_claims(Some(&serde_json::json!(1))));
        assert!(!extract_bool_from_claims(Some(&serde_json::json!("false"))));
        assert!(!extract_bool_from_claims(Some(&serde_json::json!("0"))));
        assert!(!extract_bool_from_claims(Some(&serde_json::json!(0))));
        assert!(!extract_bool_from_claims(None));
    }

    #[test]
    fn test_should_promote_to_primary() {
        // An unverified candidate does NOT displace an already-verified primary email
        assert!(!should_promote_to_primary(false, Some(true)));

        // A verified candidate DOES promote over an already-verified primary email
        assert!(should_promote_to_primary(true, Some(true)));

        // An unverified candidate DOES promote if current primary is unverified or absent
        assert!(should_promote_to_primary(false, Some(false)));
        assert!(should_promote_to_primary(false, None));

        // A verified candidate always promotes if current primary is unverified or absent
        assert!(should_promote_to_primary(true, Some(false)));
        assert!(should_promote_to_primary(true, None));
    }
}
