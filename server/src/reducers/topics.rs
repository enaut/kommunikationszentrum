use log::{error, info};
use spacetimedb::{ReducerContext, Table, Timestamp};

use crate::common::auth::{
    is_admin_identity, is_admin_or_topic_moderator, is_admin_user, is_topic_moderator,
};
use crate::models::account::{Account, account, account_emails};
use crate::models::domain::domains;
use crate::models::topic::*;
use crate::services::stalwart::topic::provision_stalwart_topic_mailbox;

#[spacetimedb::reducer]
pub fn add_message_topic(
    ctx: &ReducerContext,
    name: String,
    email_address: String,
    description: String,
    visibility: TopicVisibility,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }

    ctx.db.message_topics().insert(MessageTopic {
        id: 0,
        name,
        email_address,
        description,
        active: true,
        visibility,
        app_password_id: None,
        default_permission: SubscriptionPermission::Read,
        locked_is_provisioning: false,
    });
    log::info!("Added new message topic (by identity: {:?})", ctx.sender());
    Ok(())
}

#[spacetimedb::reducer]
pub fn remove_message_topic(ctx: &ReducerContext, topic_id: u64) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    let topic = ctx
        .db
        .message_topics()
        .id()
        .find(&topic_id)
        .ok_or_else(|| format!("Message topic {} not found", topic_id))?;
    if let Some(app_password_id) = topic.app_password_id {
        ctx.db.topic_app_passwords().id().delete(&app_password_id);
    }
    ctx.db.message_topics().id().delete(&topic_id);
    log::info!(
        "Removed message topic {} (by identity: {:?})",
        topic_id,
        ctx.sender()
    );
    Ok(())
}

/// Updates the editable fields (name, description, visibility, default_permission, locked_is_provisioning)
/// of an existing message topic. The `email_address` is immutable via this reducer since it is
/// used to route incoming mail and to match topics during user sync.
pub(crate) fn can_update_topic_default_permission(
    is_admin: bool,
    current_default: SubscriptionPermission,
    new_default: Option<SubscriptionPermission>,
) -> Result<(), &'static str> {
    if !is_admin {
        if let Some(new_perm) = new_default {
            if new_perm == SubscriptionPermission::Moderate
                && current_default != SubscriptionPermission::Moderate
            {
                return Err(
                    "Unauthorized: Only administrators can set Moderate as a topic default",
                );
            }
            if current_default == SubscriptionPermission::Moderate
                && new_perm != SubscriptionPermission::Moderate
            {
                return Err(
                    "Unauthorized: Only administrators can remove Moderate default permission",
                );
            }
        }
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_message_topic(
    ctx: &ReducerContext,
    topic_id: u64,
    name: String,
    description: String,
    visibility: Option<TopicVisibility>,
    default_permission: Option<SubscriptionPermission>,
    clear_provisioning_lock: Option<bool>,
) -> Result<(), String> {
    if !is_admin_or_topic_moderator(ctx, topic_id) {
        return Err("Unauthorized: Admin or topic moderator access required".to_string());
    }
    let existing = ctx
        .db
        .message_topics()
        .id()
        .find(&topic_id)
        .ok_or_else(|| format!("Message topic {} not found", topic_id))?;

    if name.trim().is_empty() {
        return Err("Name must not be empty".to_string());
    }

    can_update_topic_default_permission(
        is_admin_user(ctx),
        existing.default_permission,
        default_permission,
    )
    .map_err(|e| e.to_string())?;

    let mut updated = MessageTopic {
        name,
        description,
        visibility: visibility.unwrap_or(existing.visibility),
        default_permission: default_permission.unwrap_or(existing.default_permission),
        ..existing
    };

    if clear_provisioning_lock.unwrap_or(false) {
        updated.locked_is_provisioning = false;
    }

    ctx.db.message_topics().id().update(updated);
    log::info!(
        "Updated message topic {} (by identity: {:?})",
        topic_id,
        ctx.sender()
    );
    Ok(())
}

/// Clears the `locked_is_provisioning` flag on a topic if it is set.
/// Useful if an earlier provisioning run was interrupted or crashed.
#[spacetimedb::reducer]
pub fn clear_topic_provisioning_lock(ctx: &ReducerContext, topic_id: u64) -> Result<(), String> {
    if !is_admin_or_topic_moderator(ctx, topic_id) {
        return Err("Unauthorized: Admin or topic moderator access required".to_string());
    }
    let mut topic = ctx
        .db
        .message_topics()
        .id()
        .find(&topic_id)
        .ok_or_else(|| format!("Message topic {} not found", topic_id))?;

    if topic.locked_is_provisioning {
        topic.locked_is_provisioning = false;
        ctx.db.message_topics().id().update(topic);
        log::info!(
            "Cleared provisioning lock on topic {} (by identity: {:?})",
            topic_id,
            ctx.sender()
        );
    }
    Ok(())
}

/// Replaces the category assignments of a message topic. Missing category names are
/// created automatically. An empty `category_names` list clears all assignments.
/// Admin or topic moderator; used by the topic detail tag editor.
#[spacetimedb::reducer]
pub fn set_topic_categories(
    ctx: &ReducerContext,
    topic_id: u64,
    category_names: Vec<String>,
) -> Result<(), String> {
    if !is_admin_or_topic_moderator(ctx, topic_id) {
        return Err("Unauthorized: Admin or topic moderator access required".to_string());
    }
    if ctx.db.message_topics().id().find(&topic_id).is_none() {
        return Err(format!("Message topic {} not found", topic_id));
    }
    sync_topic_categories(ctx, topic_id, category_names)?;
    log::info!(
        "Updated categories for message topic {} (by identity: {:?})",
        topic_id,
        ctx.sender()
    );
    Ok(())
}

/// Renames an existing category. Admin-only; used by the category tag editor.
#[spacetimedb::reducer]
pub fn rename_category(
    ctx: &ReducerContext,
    category_id: u64,
    new_name: String,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    let new_name = new_name.trim().to_string();
    if new_name.is_empty() {
        return Err("Category name must not be empty".to_string());
    }
    let existing = ctx
        .db
        .categories()
        .id()
        .find(&category_id)
        .ok_or_else(|| format!("Category {} not found", category_id))?;
    if existing.name == new_name {
        return Ok(());
    }
    if let Some(other) = ctx.db.categories().name().find(&new_name) {
        if other.id != category_id {
            return Err(format!("Category '{new_name}' already exists"));
        }
    }
    ctx.db.categories().id().update(Category {
        name: new_name.clone(),
        ..existing
    });
    log::info!(
        "Renamed category {} to '{}' (by identity: {:?})",
        category_id,
        new_name,
        ctx.sender()
    );
    Ok(())
}

/// Core insert-or-update logic for a subscription, without any authorization
/// checks. Callable both from the admin-guarded `add_subscription` reducer and
/// from privileged internal flows (e.g. user sync) that already gate access
/// at a higher level.
pub(crate) fn do_add_subscription(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    account_email_id: u64,
    topic_id: u64,
    status: SubscriptionStatus,
    force: bool,
) -> Result<Subscription, String> {
    let timestamp = ctx.timestamp;

    let email = ctx
        .db
        .account_emails()
        .id()
        .find(&account_email_id)
        .ok_or_else(|| format!("Account email {} not found", account_email_id))?;

    if email.account_id != subscriber_account_id {
        return Err("Email does not belong to the subscriber account".to_string());
    }

    if !email.is_verified {
        return Err("Cannot subscribe an unverified email address".to_string());
    }

    let topic = ctx
        .db
        .message_topics()
        .id()
        .find(&topic_id)
        .ok_or("Topic not found")?;

    let existing = ctx
        .db
        .subscriptions()
        .subscriber_account_id()
        .filter(&subscriber_account_id)
        .find(|sub| sub.topic_id == topic_id && sub.account_email_id == account_email_id);

    let subscription = if let Some(existing) = existing {
        // When not forced and the new status is automatic, protect manual/link-unsubscribed status.
        if !force && status.is_automatic() && !existing.status.is_automatic() {
            return Ok(existing);
        }
        let permission = if topic.default_permission == SubscriptionPermission::Write
            && existing.permission == SubscriptionPermission::Read
        {
            SubscriptionPermission::Write
        } else {
            existing.permission
        };
        let updated = Subscription {
            account_email_id,
            subscribed_at: timestamp,
            status,
            permission,
            ..existing
        };
        ctx.db.subscriptions().id().update(updated.clone());
        updated
    } else {
        let candidate = Subscription {
            id: 0,
            subscriber_account_id,
            account_email_id,
            topic_id,
            subscribed_at: timestamp,
            status,
            permission: topic.default_permission,
        };
        ctx.db.subscriptions().insert(candidate);
        ctx.db
            .subscriptions()
            .subscriber_account_id()
            .filter(&subscriber_account_id)
            .find(|sub| sub.topic_id == topic_id && sub.account_email_id == account_email_id)
            .ok_or_else(|| "Subscription insert failed".to_string())?
    };

    let token = upsert_subscription_unsubscribe_token(ctx, subscription.id);
    log::info!(
        "Added subscription for account {} (token: {})",
        subscriber_account_id,
        token
    );
    Ok(subscription)
}

#[spacetimedb::reducer]
pub fn add_subscription(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    account_email_id: u64,
    topic_id: u64,
) -> Result<(), String> {
    let is_admin = is_admin_user(ctx);
    let is_moderator = is_topic_moderator(ctx, topic_id);
    let is_self = ctx
        .db
        .account()
        .id()
        .find(&subscriber_account_id)
        .map(|a: Account| a.identity == ctx.sender())
        .unwrap_or(false);

    if !is_admin && !is_self && !is_moderator {
        return Err(
            "Unauthorized: can only subscribe yourself or requires admin or topic moderator"
                .to_string(),
        );
    }

    let email = ctx
        .db
        .account_emails()
        .id()
        .find(&account_email_id)
        .ok_or_else(|| format!("Account email {} not found", account_email_id))?;

    if !is_admin && !is_moderator && !email.is_verified {
        return Err("Cannot subscribe an unverified email address".to_string());
    }

    let topic = ctx
        .db
        .message_topics()
        .id()
        .find(&topic_id)
        .ok_or("Topic not found")?;

    if !is_admin && !is_moderator && topic.visibility != TopicVisibility::Public {
        let already_subscribed = ctx
            .db
            .subscriptions()
            .subscriber_account_id()
            .filter(&subscriber_account_id)
            .any(|s| s.topic_id == topic_id && s.status.is_active());
        if !already_subscribed {
            return Err("Cannot subscribe to a private topic without an invitation".to_string());
        }
    }

    do_add_subscription(
        ctx,
        subscriber_account_id,
        account_email_id,
        topic_id,
        SubscriptionStatus::ManuallySubscribed,
        true, // force: explicit user/admin action always applies
    )?;
    Ok(())
}

pub(crate) fn can_moderator_set_subscription_status(
    is_admin: bool,
    status: SubscriptionStatus,
) -> Result<(), &'static str> {
    if !is_admin
        && status != SubscriptionStatus::ManuallySubscribed
        && status != SubscriptionStatus::ManuallyUnsubscribed
    {
        return Err(
            "Unauthorized: Moderators can only set manually subscribed or manually unsubscribed status",
        );
    }
    Ok(())
}

pub(crate) fn can_moderator_modify_existing_subscription(
    is_admin: bool,
    existing_status: SubscriptionStatus,
    existing_permission: SubscriptionPermission,
) -> Result<(), &'static str> {
    if !is_admin {
        if existing_permission.is_moderator() {
            return Err(
                "Unauthorized: Only global administrators can modify moderator subscriptions",
            );
        }
        if existing_status == SubscriptionStatus::RequiredSubscribed {
            return Err(
                "Unauthorized: Required subscriptions can only be modified by an administrator or sync",
            );
        }
    }
    Ok(())
}

/// Admin or topic moderator reducer that adds or updates a subscription with an explicitly chosen status.
#[spacetimedb::reducer]
pub fn admin_add_subscription(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    account_email_id: u64,
    topic_id: u64,
    status: SubscriptionStatus,
) -> Result<(), String> {
    let is_admin = is_admin_user(ctx);
    if !is_admin_or_topic_moderator(ctx, topic_id) {
        return Err("Unauthorized: Admin or topic moderator access required".to_string());
    }

    can_moderator_set_subscription_status(is_admin, status).map_err(|e| e.to_string())?;

    let existing = ctx
        .db
        .subscriptions()
        .subscriber_account_id()
        .filter(&subscriber_account_id)
        .find(|s| s.topic_id == topic_id && s.account_email_id == account_email_id);

    if let Some(existing) = existing {
        can_moderator_modify_existing_subscription(is_admin, existing.status, existing.permission)
            .map_err(|e| e.to_string())?;
    }

    do_add_subscription(
        ctx,
        subscriber_account_id,
        account_email_id,
        topic_id,
        status,
        true, // force: explicit admin action always overwrites existing status
    )?;
    Ok(())
}

/// Syncs the categories associated with a message topic. Missing category names are created.
pub(crate) fn sync_topic_categories(
    ctx: &ReducerContext,
    topic_id: u64,
    category_names: Vec<String>,
) -> Result<(), String> {
    let mut category_names: Vec<String> = category_names
        .into_iter()
        .map(|name| name.trim().to_string())
        .collect();
    if category_names.iter().any(|name| name.is_empty()) {
        return Err("Topic categories must not be empty".to_string());
    }
    category_names.sort();
    category_names.dedup();

    let mut desired_category_ids = Vec::with_capacity(category_names.len());
    for name in category_names {
        let category = match ctx.db.categories().name().find(&name) {
            Some(category) => category,
            None => {
                ctx.db.categories().insert(Category {
                    id: 0,
                    name: name.clone(),
                });
                ctx.db
                    .categories()
                    .name()
                    .find(&name)
                    .ok_or_else(|| "Category insert failed".to_string())?
            }
        };
        desired_category_ids.push(category.id);
    }

    let existing_links: Vec<_> = ctx
        .db
        .message_topic_categories()
        .topic_id()
        .filter(&topic_id)
        .collect();
    for link in &existing_links {
        if !desired_category_ids.contains(&link.category_id) {
            ctx.db.message_topic_categories().id().delete(&link.id);
        }
    }
    for category_id in desired_category_ids {
        if !existing_links
            .iter()
            .any(|link| link.category_id == category_id)
        {
            ctx.db
                .message_topic_categories()
                .insert(MessageTopicCategory {
                    id: 0,
                    topic_id,
                    category_id,
                });
        }
    }
    Ok(())
}

pub(crate) fn do_add_and_subscribe_topic(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    account_email_id: u64,
    name: String,
    email_address: String,
    description: String,
    visibility: String,
    categories: Option<Vec<String>>,
    required: bool,
    default_permission: Option<String>,
) -> Result<(), String> {
    let visibility = TopicVisibility::parse(&visibility)?;
    let parsed_default_permission = match default_permission.as_deref() {
        Some(p) => Some(SubscriptionPermission::parse(p)?),
        None => None,
    };
    let topic = match ctx.db.message_topics().email_address().find(&email_address) {
        Some(existing) => {
            let mut updated = existing.clone();
            let mut changed = false;
            // Visibility comes from the authoritative Django sync, unlike the
            // manually editable topic content.
            if existing.visibility != visibility {
                updated.visibility = visibility;
                changed = true;
            }
            if let Some(perm) = parsed_default_permission {
                if existing.default_permission != perm {
                    updated.default_permission = perm;
                    changed = true;
                }
            }
            if changed {
                ctx.db.message_topics().id().update(updated);
            }
            ctx.db
                .message_topics()
                .email_address()
                .find(&email_address)
                .expect("existing topic disappeared during update")
        }
        None => {
            ctx.db.message_topics().insert(MessageTopic {
                id: 0,
                name,
                email_address: email_address.clone(),
                description,
                active: true,
                visibility,
                app_password_id: None,
                default_permission: parsed_default_permission
                    .unwrap_or(SubscriptionPermission::Read),
                locked_is_provisioning: false,
            });
            ctx.db
                .message_topics()
                .email_address()
                .find(&email_address)
                .ok_or_else(|| "Topic insert failed".to_string())?
        }
    };

    if let Some(categories) = categories {
        sync_topic_categories(ctx, topic.id, categories)?;
    }

    do_add_subscription(
        ctx,
        subscriber_account_id,
        account_email_id,
        topic.id,
        if required {
            SubscriptionStatus::RequiredSubscribed
        } else {
            SubscriptionStatus::AutomaticallySubscribed
        },
        false, // force=false: sync path must not overwrite manual/link-unsubscribed status
    )?;
    Ok(())
}

/// Admin-callable reducer combining `add_message_topic` (idempotent,
/// add-only) and `add_subscription` in a single call.
#[spacetimedb::reducer]
pub fn add_and_subscribe_topic(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    account_email_id: u64,
    name: String,
    email_address: String,
    description: String,
    visibility: TopicVisibility,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    let visibility_str = match visibility {
        TopicVisibility::Public => "public".to_string(),
        TopicVisibility::Private => "private".to_string(),
    };
    do_add_and_subscribe_topic(
        ctx,
        subscriber_account_id,
        account_email_id,
        name,
        email_address,
        description,
        visibility_str,
        None,
        false,
        None,
    )
}

/// Core deactivation logic shared by manual removal, sync-driven automatic unsubscription, and
/// mail-link unsubscription.
fn do_deactivate_subscription(
    ctx: &ReducerContext,
    subscription: Subscription,
    status: SubscriptionStatus,
) -> bool {
    if status.is_automatic() && !subscription.status.is_sync_managed() {
        return false;
    }
    let sub_id = subscription.id;
    let mut updated = subscription;
    updated.status = status;
    ctx.db.subscriptions().id().update(updated);
    deactivate_subscription_unsubscribe_token(ctx, sub_id);
    true
}

/// Deactivates (without deleting) the given account's subscription to the
/// topic identified by `topic_email_address`, if both exist and the
/// subscription is currently active.
pub(crate) fn do_remove_subscription_for_topic_email(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    topic_email_address: &str,
) -> Result<(), String> {
    let Some(topic) = ctx
        .db
        .message_topics()
        .email_address()
        .find(&topic_email_address.to_string())
    else {
        return Ok(());
    };

    let subs: Vec<_> = ctx
        .db
        .subscriptions()
        .subscriber_account_id()
        .filter(&subscriber_account_id)
        .filter(|s| s.topic_id == topic.id)
        .collect();

    for sub in subs {
        if sub.status.is_active() {
            let sub_id = sub.id;
            if do_deactivate_subscription(ctx, sub, SubscriptionStatus::AutomaticallyUnsubscribed) {
                log::info!(
                    "Deactivated subscription {} for account {} (topic email: {})",
                    sub_id,
                    subscriber_account_id,
                    topic_email_address
                );
            } else {
                log::info!(
                    "Skipped sync-driven unsubscribe of subscription {} for account {} (topic email: {}): manually managed",
                    sub_id,
                    subscriber_account_id,
                    topic_email_address
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn can_remove_subscription(
    is_admin: bool,
    is_self: bool,
    is_moderator: bool,
    target_permission: SubscriptionPermission,
    target_status: SubscriptionStatus,
) -> Result<(), &'static str> {
    if !is_admin && !is_self && !is_moderator {
        return Err(
            "Unauthorized: can only remove your own subscriptions or requires admin or topic moderator",
        );
    }

    if !is_admin && !is_self && target_permission.is_moderator() {
        return Err("Unauthorized: Only global administrators can remove moderator subscriptions");
    }

    if !is_admin && target_status == SubscriptionStatus::RequiredSubscribed {
        return Err(
            "Required subscriptions can only be removed by an administrator or Django sync",
        );
    }

    Ok(())
}

#[spacetimedb::reducer]
pub fn remove_subscription(ctx: &ReducerContext, subscription_id: u64) -> Result<(), String> {
    let is_admin = is_admin_user(ctx);
    let sub = ctx
        .db
        .subscriptions()
        .id()
        .find(&subscription_id)
        .ok_or_else(|| format!("Subscription {} not found", subscription_id))?;
    let is_self = ctx
        .db
        .account()
        .id()
        .find(&sub.subscriber_account_id)
        .map(|a| a.identity == ctx.sender())
        .unwrap_or(false);
    let is_moderator = is_topic_moderator(ctx, sub.topic_id);

    can_remove_subscription(is_admin, is_self, is_moderator, sub.permission, sub.status)
        .map_err(|e| e.to_string())?;

    let sub = ctx
        .db
        .subscriptions()
        .id()
        .find(&subscription_id)
        .ok_or_else(|| format!("Subscription {} not found", subscription_id))?;

    do_deactivate_subscription(ctx, sub, SubscriptionStatus::ManuallyUnsubscribed);
    log::info!(
        "Deactivated subscription {} (by identity: {:?})",
        subscription_id,
        ctx.sender()
    );
    Ok(())
}

pub(crate) fn can_update_permission(
    is_admin: bool,
    is_moderator: bool,
    current_permission: SubscriptionPermission,
    new_permission: SubscriptionPermission,
) -> Result<(), &'static str> {
    if !is_admin && !is_moderator {
        return Err("Unauthorized: Admin or topic moderator access required");
    }

    if current_permission == new_permission {
        return Ok(());
    }

    // Only global admins can grant or revoke Moderate permission
    if !is_admin
        && (new_permission == SubscriptionPermission::Moderate
            || current_permission == SubscriptionPermission::Moderate)
    {
        return Err(
            "Unauthorized: Only global administrators can assign or revoke moderation rights",
        );
    }

    Ok(())
}

#[spacetimedb::reducer]
pub fn update_subscription_permission(
    ctx: &ReducerContext,
    subscription_id: u64,
    permission: SubscriptionPermission,
) -> Result<(), String> {
    let existing = ctx
        .db
        .subscriptions()
        .id()
        .find(&subscription_id)
        .ok_or_else(|| format!("Subscription {} not found", subscription_id))?;

    let is_admin = is_admin_user(ctx);
    let is_moderator = is_topic_moderator(ctx, existing.topic_id);

    can_update_permission(is_admin, is_moderator, existing.permission, permission)
        .map_err(|e| e.to_string())?;

    let updated = Subscription {
        permission,
        ..existing
    };
    ctx.db.subscriptions().id().update(updated);
    log::info!(
        "Updated permission for subscription {} to {:?} (by identity: {:?})",
        subscription_id,
        permission,
        ctx.sender()
    );
    Ok(())
}

fn upsert_subscription_unsubscribe_token(ctx: &ReducerContext, subscription_id: u64) -> String {
    if let Some(existing) = ctx
        .db
        .subscription_unsubscribe_tokens()
        .subscription_id()
        .find(&subscription_id)
    {
        if existing.active {
            return existing.token;
        }

        let mut updated = existing.clone();
        updated.active = true;
        updated.revoked_at = Timestamp::UNIX_EPOCH;
        updated.created_at = ctx.timestamp;
        ctx.db
            .subscription_unsubscribe_tokens()
            .token()
            .update(updated.clone());
        return updated.token;
    }

    let token = format!("sub-{subscription_id}-{:032x}", ctx.random::<u128>());
    ctx.db
        .subscription_unsubscribe_tokens()
        .insert(SubscriptionUnsubscribeToken {
            token: token.clone(),
            subscription_id,
            created_at: ctx.timestamp,
            active: true,
            revoked_at: Timestamp::UNIX_EPOCH,
        });
    token
}

fn deactivate_subscription_unsubscribe_token(ctx: &ReducerContext, subscription_id: u64) {
    if let Some(existing) = ctx
        .db
        .subscription_unsubscribe_tokens()
        .subscription_id()
        .find(&subscription_id)
    {
        let mut updated = existing.clone();
        updated.active = false;
        updated.revoked_at = ctx.timestamp;
        ctx.db
            .subscription_unsubscribe_tokens()
            .token()
            .update(updated);
    }
}

#[spacetimedb::reducer]
pub fn ensure_subscription_unsubscribe_token(ctx: &ReducerContext, subscription_id: u64) -> () {
    upsert_subscription_unsubscribe_token(ctx, subscription_id);
}

#[spacetimedb::reducer]
pub fn user_unsubscribe_by_token(ctx: &ReducerContext, token: String) -> Result<(), String> {
    unsubscribe_subscription_by_token(ctx, token)
}

pub(crate) fn unsubscribe_subscription_by_token(
    ctx: &ReducerContext,
    token: String,
) -> Result<(), String> {
    let token_row = ctx
        .db
        .subscription_unsubscribe_tokens()
        .token()
        .find(&token)
        .ok_or_else(|| "Unknown unsubscribe token".to_string())?;

    let Some(subscription) = ctx.db.subscriptions().id().find(&token_row.subscription_id) else {
        return Err("Subscription missing for token".to_string());
    };

    if !subscription.status.is_active() {
        return Ok(());
    }

    if subscription.status == SubscriptionStatus::RequiredSubscribed {
        return Err(
            "Required subscriptions cannot be removed using an unsubscribe link".to_string(),
        );
    }

    do_deactivate_subscription(ctx, subscription, SubscriptionStatus::LinkUnsubscribed);
    Ok(())
}

// Procedure: Provision a Stalwart mailbox via JMAP and insert the message topic on success.
#[spacetimedb::procedure]
pub fn provision_message_topic(
    ctx: &mut spacetimedb::ProcedureContext,
    name: String,
    base: String,
    domain_id: String,
    description: String,
    visibility: TopicVisibility,
) -> Result<(), String> {
    info!(
        "Provisioning a new Topic: name={}, base={}, domain_id={}, description={}",
        name, base, domain_id, description
    );
    // 1) Authorization check: capture the procedure caller identity and check inside a transaction
    let caller = ctx.sender();
    info!("Checking permissions for identity: {:?}", caller);
    let is_admin: bool = ctx.with_tx(|tx| is_admin_identity(tx, caller));

    if !is_admin {
        return Err("Unauthorized: Admin access required".to_string());
    }

    info!("User has required permissions!");

    // 2) Look up domain to construct the full email address
    let domain = ctx.with_tx(|tx| tx.db.domains().id().find(&domain_id));
    let domain = domain.ok_or_else(|| format!("Domain with id '{}' not found", domain_id))?;
    let email_address = format!("{}@{}", base.trim(), domain.name.trim());

    // 3) Ensure topic doesn't already exist
    let exists: bool = ctx.with_tx(|tx| {
        tx.db
            .message_topics()
            .email_address()
            .find(&email_address)
            .is_some()
    });

    if exists {
        error!("The topic with that email address already exists");
        return Err(format!("Topic with email {} already exists", email_address));
    }

    provision_stalwart_topic_mailbox(
        ctx,
        &name,
        &email_address,
        &description,
        visibility,
        SubscriptionPermission::Read,
    )?;

    Ok(())
}

/// Admin Procedure: Provisions all existing topics in message_topics that have app_password_id == None.
#[spacetimedb::procedure]
pub fn provision_all_unprovisioned_topics(
    ctx: &mut spacetimedb::ProcedureContext,
) -> Result<u32, String> {
    info!("Executing provision_all_unprovisioned_topics procedure");

    let caller = ctx.sender();
    let is_admin: bool = ctx.with_tx(|tx| is_admin_identity(tx, caller));
    if !is_admin {
        return Err("Unauthorized: Admin access required".to_string());
    }

    // Collect all unprovisioned topics in a transaction
    let unprovisioned: Vec<(
        String,
        String,
        String,
        TopicVisibility,
        SubscriptionPermission,
    )> = ctx.with_tx(|tx| {
        tx.db
            .message_topics()
            .iter()
            .filter(|c| c.app_password_id.is_none())
            .map(|c| {
                (
                    c.name.clone(),
                    c.email_address.clone(),
                    c.description.clone(),
                    c.visibility,
                    c.default_permission,
                )
            })
            .collect()
    });

    info!("Found {} unprovisioned topics", unprovisioned.len());
    let mut provisioned_count = 0u32;

    for (name, email_address, description, visibility, default_permission) in unprovisioned {
        info!("Provisioning topic '{}' ({})", name, email_address);
        match provision_stalwart_topic_mailbox(
            ctx,
            &name,
            &email_address,
            &description,
            visibility,
            default_permission,
        ) {
            Ok(_) => {
                provisioned_count += 1;
            }
            Err(e) => {
                error!(
                    "Failed to provision topic '{}' ({}): {}",
                    name, email_address, e
                );
                return Err(format!(
                    "Failed to provision topic '{}' ({}): {}. (Successfully provisioned {} before failure)",
                    name, email_address, e, provisioned_count
                ));
            }
        }
    }

    info!(
        "Successfully provisioned {} unprovisioned topics",
        provisioned_count
    );
    Ok(provisioned_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_permission_updates() {
        assert!(
            can_update_permission(
                true,
                false,
                SubscriptionPermission::Read,
                SubscriptionPermission::Write
            )
            .is_ok()
        );
        assert!(
            can_update_permission(
                true,
                false,
                SubscriptionPermission::Write,
                SubscriptionPermission::Read
            )
            .is_ok()
        );
        assert!(
            can_update_permission(
                true,
                false,
                SubscriptionPermission::Read,
                SubscriptionPermission::Moderate
            )
            .is_ok()
        );
        assert!(
            can_update_permission(
                true,
                false,
                SubscriptionPermission::Write,
                SubscriptionPermission::Moderate
            )
            .is_ok()
        );
        assert!(
            can_update_permission(
                true,
                false,
                SubscriptionPermission::Moderate,
                SubscriptionPermission::Write
            )
            .is_ok()
        );
        assert!(
            can_update_permission(
                true,
                false,
                SubscriptionPermission::Moderate,
                SubscriptionPermission::Read
            )
            .is_ok()
        );
    }

    #[test]
    fn test_moderator_permission_updates() {
        assert!(
            can_update_permission(
                false,
                true,
                SubscriptionPermission::Read,
                SubscriptionPermission::Write
            )
            .is_ok()
        );
        assert!(
            can_update_permission(
                false,
                true,
                SubscriptionPermission::Write,
                SubscriptionPermission::Read
            )
            .is_ok()
        );

        // Moderator preserving Moderate (unchanged) is allowed
        assert!(
            can_update_permission(
                false,
                true,
                SubscriptionPermission::Moderate,
                SubscriptionPermission::Moderate
            )
            .is_ok()
        );

        // Moderator cannot grant Moderate
        assert!(
            can_update_permission(
                false,
                true,
                SubscriptionPermission::Read,
                SubscriptionPermission::Moderate
            )
            .is_err()
        );
        assert!(
            can_update_permission(
                false,
                true,
                SubscriptionPermission::Write,
                SubscriptionPermission::Moderate
            )
            .is_err()
        );

        // Moderator cannot revoke Moderate
        assert!(
            can_update_permission(
                false,
                true,
                SubscriptionPermission::Moderate,
                SubscriptionPermission::Write
            )
            .is_err()
        );
        assert!(
            can_update_permission(
                false,
                true,
                SubscriptionPermission::Moderate,
                SubscriptionPermission::Read
            )
            .is_err()
        );
    }

    #[test]
    fn test_non_moderator_permission_updates() {
        assert!(
            can_update_permission(
                false,
                false,
                SubscriptionPermission::Read,
                SubscriptionPermission::Write
            )
            .is_err()
        );
        assert!(
            can_update_permission(
                false,
                false,
                SubscriptionPermission::Write,
                SubscriptionPermission::Read
            )
            .is_err()
        );
        assert!(
            can_update_permission(
                false,
                false,
                SubscriptionPermission::Read,
                SubscriptionPermission::Moderate
            )
            .is_err()
        );
    }

    #[test]
    fn test_default_permission_restrictions() {
        // Admin can set or remove Moderate default permission
        assert!(
            can_update_topic_default_permission(
                true,
                SubscriptionPermission::Read,
                Some(SubscriptionPermission::Moderate)
            )
            .is_ok()
        );
        assert!(
            can_update_topic_default_permission(
                true,
                SubscriptionPermission::Moderate,
                Some(SubscriptionPermission::Read)
            )
            .is_ok()
        );

        // Moderator cannot set Moderate default permission
        assert!(
            can_update_topic_default_permission(
                false,
                SubscriptionPermission::Read,
                Some(SubscriptionPermission::Moderate)
            )
            .is_err()
        );
        assert!(
            can_update_topic_default_permission(
                false,
                SubscriptionPermission::Write,
                Some(SubscriptionPermission::Moderate)
            )
            .is_err()
        );

        // Moderator cannot remove Moderate default permission
        assert!(
            can_update_topic_default_permission(
                false,
                SubscriptionPermission::Moderate,
                Some(SubscriptionPermission::Read)
            )
            .is_err()
        );
        assert!(
            can_update_topic_default_permission(
                false,
                SubscriptionPermission::Moderate,
                Some(SubscriptionPermission::Write)
            )
            .is_err()
        );

        // Moderator can keep Moderate default permission unchanged
        assert!(
            can_update_topic_default_permission(
                false,
                SubscriptionPermission::Moderate,
                Some(SubscriptionPermission::Moderate)
            )
            .is_ok()
        );
        assert!(
            can_update_topic_default_permission(false, SubscriptionPermission::Moderate, None)
                .is_ok()
        );

        // Moderator can change between Read and Write
        assert!(
            can_update_topic_default_permission(
                false,
                SubscriptionPermission::Read,
                Some(SubscriptionPermission::Write)
            )
            .is_ok()
        );
        assert!(
            can_update_topic_default_permission(
                false,
                SubscriptionPermission::Write,
                Some(SubscriptionPermission::Read)
            )
            .is_ok()
        );
    }

    #[test]
    fn test_moderator_add_subscription_status() {
        // Admin can set any status
        assert!(
            can_moderator_set_subscription_status(true, SubscriptionStatus::ManuallySubscribed)
                .is_ok()
        );
        assert!(
            can_moderator_set_subscription_status(true, SubscriptionStatus::ManuallyUnsubscribed)
                .is_ok()
        );
        assert!(
            can_moderator_set_subscription_status(true, SubscriptionStatus::RequiredSubscribed)
                .is_ok()
        );
        assert!(
            can_moderator_set_subscription_status(
                true,
                SubscriptionStatus::AutomaticallySubscribed
            )
            .is_ok()
        );

        // Moderator can only set ManuallySubscribed or ManuallyUnsubscribed
        assert!(
            can_moderator_set_subscription_status(false, SubscriptionStatus::ManuallySubscribed)
                .is_ok()
        );
        assert!(
            can_moderator_set_subscription_status(false, SubscriptionStatus::ManuallyUnsubscribed)
                .is_ok()
        );
        assert!(
            can_moderator_set_subscription_status(false, SubscriptionStatus::RequiredSubscribed)
                .is_err()
        );
        assert!(
            can_moderator_set_subscription_status(
                false,
                SubscriptionStatus::AutomaticallySubscribed
            )
            .is_err()
        );
        assert!(
            can_moderator_set_subscription_status(false, SubscriptionStatus::LinkUnsubscribed)
                .is_err()
        );
    }

    #[test]
    fn test_moderator_modify_existing_subscription() {
        // Admin can modify any subscription
        assert!(
            can_moderator_modify_existing_subscription(
                true,
                SubscriptionStatus::RequiredSubscribed,
                SubscriptionPermission::Moderate
            )
            .is_ok()
        );

        // Moderator cannot modify moderator subscription
        assert!(
            can_moderator_modify_existing_subscription(
                false,
                SubscriptionStatus::ManuallySubscribed,
                SubscriptionPermission::Moderate
            )
            .is_err()
        );

        // Moderator cannot modify RequiredSubscribed subscription
        assert!(
            can_moderator_modify_existing_subscription(
                false,
                SubscriptionStatus::RequiredSubscribed,
                SubscriptionPermission::Read
            )
            .is_err()
        );

        // Moderator can modify standard subscription
        assert!(
            can_moderator_modify_existing_subscription(
                false,
                SubscriptionStatus::ManuallySubscribed,
                SubscriptionPermission::Read
            )
            .is_ok()
        );
        assert!(
            can_moderator_modify_existing_subscription(
                false,
                SubscriptionStatus::ManuallyUnsubscribed,
                SubscriptionPermission::Write
            )
            .is_ok()
        );
    }

    #[test]
    fn test_can_remove_subscription() {
        // Admin can remove any subscription
        assert!(
            can_remove_subscription(
                true,
                false,
                false,
                SubscriptionPermission::Moderate,
                SubscriptionStatus::RequiredSubscribed
            )
            .is_ok()
        );

        // Self can remove own subscription, unless it is RequiredSubscribed
        assert!(
            can_remove_subscription(
                false,
                true,
                false,
                SubscriptionPermission::Read,
                SubscriptionStatus::ManuallySubscribed
            )
            .is_ok()
        );
        assert!(
            can_remove_subscription(
                false,
                true,
                false,
                SubscriptionPermission::Read,
                SubscriptionStatus::RequiredSubscribed
            )
            .is_err()
        );

        // Moderator can remove ordinary subscriber
        assert!(
            can_remove_subscription(
                false,
                false,
                true,
                SubscriptionPermission::Read,
                SubscriptionStatus::ManuallySubscribed
            )
            .is_ok()
        );
        assert!(
            can_remove_subscription(
                false,
                false,
                true,
                SubscriptionPermission::Write,
                SubscriptionStatus::AutomaticallySubscribed
            )
            .is_ok()
        );

        // Moderator cannot remove peer moderator
        assert!(
            can_remove_subscription(
                false,
                false,
                true,
                SubscriptionPermission::Moderate,
                SubscriptionStatus::ManuallySubscribed
            )
            .is_err()
        );

        // Moderator cannot remove RequiredSubscribed
        assert!(
            can_remove_subscription(
                false,
                false,
                true,
                SubscriptionPermission::Read,
                SubscriptionStatus::RequiredSubscribed
            )
            .is_err()
        );

        // Unrelated user cannot remove someone else's subscription
        assert!(
            can_remove_subscription(
                false,
                false,
                false,
                SubscriptionPermission::Read,
                SubscriptionStatus::ManuallySubscribed
            )
            .is_err()
        );
    }
}
