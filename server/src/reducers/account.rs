use serde::{Deserialize, Serialize};
use spacetimedb::{Identity, ReducerContext, Table};

use crate::common::auth::{is_admin_identity, is_admin_user};
use crate::models::account::*;
use crate::models::topic::{subscription_unsubscribe_tokens, subscriptions};

use crate::models::delivery::*;

#[derive(Serialize, Deserialize)]
pub struct UserSyncData {
    pub mitgliedsnr: u64,
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<bool>,
    pub is_admin: Option<bool>,
    pub updated_at: Option<String>,
    pub identity_hex: Option<String>,
    #[serde(default, alias = "categories")]
    pub topics: Option<Vec<crate::models::topic::TopicSyncData>>,
    #[serde(default, alias = "unsubscribe_category_emails")]
    pub unsubscribe_topic_emails: Option<Vec<String>>,
    pub account_emails: Option<Vec<String>>,
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

                let primary_email_id = if let Some(existing_email) = ctx
                    .db
                    .account_emails()
                    .account_id()
                    .filter(&data.mitgliedsnr)
                    .find(|e| e.email == subscriber_email)
                {
                    if existing_email.source != EmailSource::DjangoSync {
                        let mut updated = existing_email.clone();
                        updated.source = EmailSource::DjangoSync;
                        ctx.db.account_emails().id().update(updated);
                    }
                    existing_email.id
                } else {
                    let new_email = ctx.db.account_emails().insert(AccountEmail {
                        id: 0,
                        account_id: data.mitgliedsnr,
                        email: subscriber_email.clone(),
                        source: EmailSource::DjangoSync,
                        is_verified: true,
                        added_at: timestamp,
                    });
                    new_email.id
                };

                if let Some(existing) = ctx.db.account().id().find(&data.mitgliedsnr) {
                    let updated = Account {
                        identity: identity_of_user,
                        name: data.name.unwrap_or_default(),
                        primary_email_id,
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
                        primary_email_id,
                        is_active: data.is_active.unwrap_or(true),
                        last_synced: timestamp,
                    };
                    log::info!("Inserting new account: {:#?}", account);
                    ctx.db.account().insert(account);
                    log::info!("Inserted new account: {}", data.mitgliedsnr);
                }

                if ctx.db.account_configs().account_id().find(&data.mitgliedsnr).is_none() {
                    ctx.db.account_configs().insert(crate::models::account::AccountConfig {
                        account_id: data.mitgliedsnr,
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

                // Sync alternative emails from Django
                let mut incoming_emails = data.account_emails.unwrap_or_default();
                incoming_emails.push(subscriber_email.clone());
                
                // 1. Remove DjangoSync emails not in the payload
                let existing_emails: Vec<_> = ctx.db.account_emails().account_id().filter(&data.mitgliedsnr).collect();
                for existing in existing_emails {
                    if existing.source == EmailSource::DjangoSync && !incoming_emails.contains(&existing.email) {
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
                                .filter(&data.mitgliedsnr)
                                .any(|s| s.topic_id == sub.topic_id && s.account_email_id == primary_email_id);

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
                                    data.mitgliedsnr,
                                    existing.email
                                );
                            } else {
                                sub.account_email_id = primary_email_id;
                                ctx.db.subscriptions().id().update(sub.clone());
                                log::info!(
                                    "Migrated subscription {} to primary_email_id {} for account {} after email {} removed",
                                    sub.id,
                                    primary_email_id,
                                    data.mitgliedsnr,
                                    existing.email
                                );
                            }
                        }

                        ctx.db.account_emails().id().delete(&existing.id);
                        log::info!("Removed old DjangoSync email: {}", existing.email);
                    }
                }
                
                // 2. Add or update incoming emails
                for incoming in incoming_emails {
                    if let Some(existing) = ctx
                        .db
                        .account_emails()
                        .account_id()
                        .filter(&data.mitgliedsnr)
                        .find(|e| e.email == incoming)
                    {
                        if existing.source != EmailSource::DjangoSync {
                            let mut updated = existing;
                            updated.source = EmailSource::DjangoSync;
                            ctx.db.account_emails().id().update(updated);
                        }
                    } else {
                        ctx.db.account_emails().insert(AccountEmail {
                            id: 0,
                            account_id: data.mitgliedsnr,
                            email: incoming.clone(),
                            source: EmailSource::DjangoSync,
                            is_verified: true,
                            added_at: timestamp,
                        });
                        log::info!("Added new DjangoSync email: {}", incoming);
                    }
                }

                for topic in data.topics.unwrap_or_default() {
                    let topic_email = topic.email_address.clone();
                    if let Err(e) = crate::reducers::topics::do_add_and_subscribe_topic(
                        ctx,
                        data.mitgliedsnr,
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
                            data.mitgliedsnr,
                            e
                        );
                    }
                }

                for topic_email in data.unsubscribe_topic_emails.unwrap_or_default() {
                    if let Err(e) =
                        crate::reducers::topics::do_remove_subscription_for_topic_email(
                            ctx,
                            data.mitgliedsnr,
                            &topic_email,
                        )
                    {
                        log::error!(
                            "Failed to remove subscription to topic '{}' for account {}: {}",
                            topic_email,
                            data.mitgliedsnr,
                            e
                        );
                    }
                }
            }
            "delete" => {
                let account_id = data.mitgliedsnr;
                if let Some(existing) = ctx.db.account().id().find(&account_id) {
                    let identity_of_user = existing.identity;
                    ctx.db.account().delete(existing);
                    log::info!("Deleted user: {} ({})", account_id, action);
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
                            account_id
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
                    log::info!("Removed subscriptions for deleted account: {}", account_id);

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
                    log::info!("Removed account emails for deleted account: {}", account_id);

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
                        account_id
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
pub fn admin_add_account_email(ctx: &ReducerContext, account_id: u64, email: String) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized".into());
    }

    if ctx.db.account_emails().account_id().filter(&account_id).any(|e| e.email == email) {
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
    let account = ctx.db.account().identity().find(&ctx.sender())
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
        format!("{:?}:{}:{}", ctx.timestamp, account.id, email).as_bytes()
    );
    let token = hex::encode(hash.data.as_slice());

    // Create verification token (expires in 24h)
    ctx.db.email_verification_tokens().insert(EmailVerificationToken {
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
    let body_text = format!("Please verify your email address by clicking the following link:\n\n{}", link);

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
    let verification = ctx.db.email_verification_tokens().token().find(&token)
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
    let email_row = ctx.db.account_emails().id().find(&account_email_id).ok_or("Not found")?;
    
    if email_row.source == EmailSource::DjangoSync {
        return Err("Cannot remove emails synced from the central database. Please remove it in your profile settings.".into());
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
    let subs: Vec<_> = ctx.db.subscriptions().account_email_id().filter(&account_email_id).collect();
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

    if let Some(val) = language { config.language = Some(val); }
    if let Some(val) = theme { config.theme = Some(val); }

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

    if ctx.db.account_configs().account_id().find(&account.id).is_some() {
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
    fn test_user_sync_data_backward_compat_aliases() {
        let legacy_json = r#"{
            "mitgliedsnr": 42,
            "name": "Erika Mustermann",
            "email": "erika@example.org",
            "categories": [
                {
                    "name": "VP Nord",
                    "email_address": "vp-nord@solawi.org",
                    "description": "Verteilpunkt Nord",
                    "topics": ["Verteilpunkt"]
                }
            ],
            "unsubscribe_category_emails": ["vp-sued@solawi.org"]
        }"#;

        let data: UserSyncData = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(data.mitgliedsnr, 42);
        let topics = data.topics.unwrap();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].name, "VP Nord");
        assert_eq!(topics[0].categories, Some(vec!["Verteilpunkt".to_string()]));
        assert_eq!(
            data.unsubscribe_topic_emails,
            Some(vec!["vp-sued@solawi.org".to_string()])
        );

        let modern_json = r#"{
            "mitgliedsnr": 43,
            "name": "Max Mustermann",
            "email": "max@example.org",
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

        let data: UserSyncData = serde_json::from_str(modern_json).unwrap();
        assert_eq!(data.mitgliedsnr, 43);
        let topics = data.topics.unwrap();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].name, "VP Süd");
        assert_eq!(topics[0].categories, Some(vec!["Verteilpunkt".to_string()]));
        assert_eq!(
            data.unsubscribe_topic_emails,
            Some(vec!["vp-nord@solawi.org".to_string()])
        );
    }
}
