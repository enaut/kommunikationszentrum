use log::{error, info};
use spacetimedb::{ReducerContext, Table, Timestamp};

use crate::common::auth::{is_admin_identity, is_admin_user};
use crate::models::account::{account, Account};
use crate::models::category::*;
use crate::models::domain::domains;
use crate::services::stalwart::category::{
    jmap_check_not_created, jmap_created_id, jmap_method_result_by_name,
    provision_stalwart_app_password,
};
use crate::services::stalwart::client::send_stalwart_jmap_request;

#[spacetimedb::reducer]
pub fn add_message_category(
    ctx: &ReducerContext,
    name: String,
    email_address: String,
    description: String,
    visibility: CategoryVisibility,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }

    ctx.db.message_categories().insert(MessageCategory {
        id: 0,
        name,
        email_address,
        description,
        active: true,
        visibility,
        app_password_id: None,
    });
    log::info!(
        "Added new message category (by identity: {:?})",
        ctx.sender()
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn remove_message_category(ctx: &ReducerContext, category_id: u64) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    let category = ctx
        .db
        .message_categories()
        .id()
        .find(&category_id)
        .ok_or_else(|| format!("Message category {} not found", category_id))?;
    if let Some(app_password_id) = category.app_password_id {
        ctx.db
            .category_app_passwords()
            .id()
            .delete(&app_password_id);
    }
    ctx.db.message_categories().id().delete(&category_id);
    log::info!(
        "Removed message category {} (by identity: {:?})",
        category_id,
        ctx.sender()
    );
    Ok(())
}

/// Updates the editable fields (name, description, visibility) of an existing message
/// category. The `email_address` is immutable via this reducer since it is
/// used to route incoming mail and to match categories during user sync.
#[spacetimedb::reducer]
pub fn update_message_category(
    ctx: &ReducerContext,
    category_id: u64,
    name: String,
    description: String,
    visibility: Option<CategoryVisibility>,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    let existing = ctx
        .db
        .message_categories()
        .id()
        .find(&category_id)
        .ok_or_else(|| format!("Message category {} not found", category_id))?;

    if name.trim().is_empty() {
        return Err("Name must not be empty".to_string());
    }

    let updated = MessageCategory {
        name,
        description,
        visibility: visibility.unwrap_or(existing.visibility),
        ..existing
    };
    ctx.db.message_categories().id().update(updated);
    log::info!(
        "Updated message category {} (by identity: {:?})",
        category_id,
        ctx.sender()
    );
    Ok(())
}

/// Replaces the topic assignments of a message category. Missing topic names are
/// created automatically. An empty `topic_names` list clears all assignments.
/// Admin-only; used by the category detail tag editor.
#[spacetimedb::reducer]
pub fn set_category_topics(
    ctx: &ReducerContext,
    category_id: u64,
    topic_names: Vec<String>,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    if ctx
        .db
        .message_categories()
        .id()
        .find(&category_id)
        .is_none()
    {
        return Err(format!("Message category {} not found", category_id));
    }
    sync_category_topics(ctx, category_id, topic_names)?;
    log::info!(
        "Updated topics for message category {} (by identity: {:?})",
        category_id,
        ctx.sender()
    );
    Ok(())
}

/// Renames an existing topic. Admin-only; used by the category detail tag editor.
#[spacetimedb::reducer]
pub fn rename_topic(ctx: &ReducerContext, topic_id: u64, new_name: String) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    let new_name = new_name.trim().to_string();
    if new_name.is_empty() {
        return Err("Topic name must not be empty".to_string());
    }
    let existing = ctx
        .db
        .topics()
        .id()
        .find(&topic_id)
        .ok_or_else(|| format!("Topic {} not found", topic_id))?;
    if existing.name == new_name {
        return Ok(());
    }
    if let Some(other) = ctx.db.topics().name().find(&new_name) {
        if other.id != topic_id {
            return Err(format!("Topic '{new_name}' already exists"));
        }
    }
    ctx.db.topics().id().update(Topic {
        name: new_name.clone(),
        ..existing
    });
    log::info!(
        "Renamed topic {} to '{}' (by identity: {:?})",
        topic_id,
        new_name,
        ctx.sender()
    );
    Ok(())
}

/// Core insert-or-update logic for a subscription, without any authorization
/// checks. Callable both from the admin-guarded `add_subscription` reducer and
/// from privileged internal flows (e.g. user sync) that already gate access
/// at a higher level.
///
/// `status` is the status to apply. When `force` is `false` and the requested status is
/// automatic, an existing subscription that is currently in a manual or link-unsubscribed
/// status is left completely untouched (returned as-is). A required subscription is always
/// applied by the sync path, including over an existing manual state.
/// When `force` is `true` (explicit admin/member action), the status is always applied
/// unconditionally, overwriting any existing status.
pub(crate) fn do_add_subscription(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    subscriber_email: String,
    category_id: u64,
    status: SubscriptionStatus,
    force: bool,
) -> Result<Subscription, String> {
    let timestamp = ctx.timestamp;

    let existing = ctx
        .db
        .subscriptions()
        .subscriber_account_id()
        .filter(&subscriber_account_id)
        .find(|sub| sub.category_id == category_id);

    let subscription = if let Some(existing) = existing {
        // When not forced and the new status is automatic, protect manual/link-unsubscribed state.
        if !force && status.is_automatic() && !existing.status.is_automatic() {
            return Ok(existing);
        }
        let updated = Subscription {
            subscriber_email: subscriber_email.clone(),
            subscribed_at: timestamp,
            status,
            ..existing
        };
        ctx.db.subscriptions().id().update(updated.clone());
        updated
    } else {
        let candidate = Subscription {
            id: 0,
            subscriber_account_id,
            subscriber_email: subscriber_email.clone(),
            category_id,
            subscribed_at: timestamp,
            status,
        };
        ctx.db.subscriptions().insert(candidate);
        ctx.db
            .subscriptions()
            .subscriber_account_id()
            .filter(&subscriber_account_id)
            .find(|sub| sub.category_id == category_id)
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
    subscriber_email: String,
    category_id: u64,
) -> Result<(), String> {
    let is_admin = is_admin_user(ctx);
    let is_self = ctx
        .db
        .account()
        .id()
        .find(&subscriber_account_id)
        .map(|a: Account| a.identity == ctx.sender())
        .unwrap_or(false);

    if !is_admin && !is_self {
        return Err("Unauthorized: can only subscribe yourself or requires admin".to_string());
    }

    do_add_subscription(
        ctx,
        subscriber_account_id,
        subscriber_email,
        category_id,
        SubscriptionStatus::ManuallySubscribed,
        true, // force: explicit user/admin action always applies
    )?;
    Ok(())
}

/// Admin-only reducer that adds or updates a subscription with an explicitly chosen status.
/// Unlike `add_subscription` (which always uses `ManuallySubscribed`), this lets admins set
/// any status — useful for pre-marking someone as `ManuallyUnsubscribed` or correcting state.
/// Because this is an explicit admin action, it always overwrites the existing status (force=true).
#[spacetimedb::reducer]
pub fn admin_add_subscription(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    subscriber_email: String,
    category_id: u64,
    status: SubscriptionStatus,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    do_add_subscription(
        ctx,
        subscriber_account_id,
        subscriber_email,
        category_id,
        status,
        true, // force: explicit admin action always overwrites existing status
    )?;
    Ok(())
}

/// Ensures a message category exists for the given `email_address` (creating
/// it if necessary, without modifying an already-existing category), then
/// subscribes the given account to it. Categories are only ever added by this
/// path, never updated or removed, so manual admin edits to an existing
/// category are never overwritten by a sync.
fn sync_category_topics(
    ctx: &ReducerContext,
    category_id: u64,
    topic_names: Vec<String>,
) -> Result<(), String> {
    let mut topic_names: Vec<String> = topic_names
        .into_iter()
        .map(|name| name.trim().to_string())
        .collect();
    if topic_names.iter().any(|name| name.is_empty()) {
        return Err("Category topics must not be empty".to_string());
    }
    topic_names.sort();
    topic_names.dedup();

    let mut desired_topic_ids = Vec::with_capacity(topic_names.len());
    for name in topic_names {
        let topic = match ctx.db.topics().name().find(&name) {
            Some(topic) => topic,
            None => {
                ctx.db.topics().insert(Topic {
                    id: 0,
                    name: name.clone(),
                });
                ctx.db
                    .topics()
                    .name()
                    .find(&name)
                    .ok_or_else(|| "Topic insert failed".to_string())?
            }
        };
        desired_topic_ids.push(topic.id);
    }

    let existing_links: Vec<_> = ctx
        .db
        .message_category_topics()
        .category_id()
        .filter(&category_id)
        .collect();
    for link in &existing_links {
        if !desired_topic_ids.contains(&link.topic_id) {
            ctx.db.message_category_topics().id().delete(&link.id);
        }
    }
    for topic_id in desired_topic_ids {
        if !existing_links.iter().any(|link| link.topic_id == topic_id) {
            ctx.db
                .message_category_topics()
                .insert(MessageCategoryTopic {
                    id: 0,
                    category_id,
                    topic_id,
                });
        }
    }
    Ok(())
}

pub(crate) fn do_add_and_subscribe_category(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    subscriber_email: String,
    name: String,
    email_address: String,
    description: String,
    visibility: String,
    topics: Option<Vec<String>>,
    required: bool,
) -> Result<(), String> {
    let visibility = CategoryVisibility::parse(&visibility)?;
    let category = match ctx
        .db
        .message_categories()
        .email_address()
        .find(&email_address)
    {
        Some(existing) => {
            // Visibility comes from the authoritative Django sync, unlike the
            // manually editable category content.
            if existing.visibility != visibility {
                ctx.db.message_categories().id().update(MessageCategory {
                    visibility,
                    ..existing
                });
            }
            ctx.db
                .message_categories()
                .email_address()
                .find(&email_address)
                .expect("existing category disappeared during update")
        }
        None => {
            ctx.db.message_categories().insert(MessageCategory {
                id: 0,
                name,
                email_address: email_address.clone(),
                description,
                active: true,
                visibility,
                app_password_id: None,
            });
            ctx.db
                .message_categories()
                .email_address()
                .find(&email_address)
                .ok_or_else(|| "Category insert failed".to_string())?
        }
    };

    if let Some(topics) = topics {
        sync_category_topics(ctx, category.id, topics)?;
    }

    do_add_subscription(
        ctx,
        subscriber_account_id,
        subscriber_email,
        category.id,
        if required {
            SubscriptionStatus::RequiredSubscribed
        } else {
            SubscriptionStatus::AutomaticallySubscribed
        },
        false, // force=false: sync path must not overwrite manual/link-unsubscribed status
    )?;
    Ok(())
}

/// Admin-callable reducer combining `add_message_category` (idempotent,
/// add-only) and `add_subscription` in a single call.
#[spacetimedb::reducer]
pub fn add_and_subscribe_category(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    subscriber_email: String,
    name: String,
    email_address: String,
    description: String,
    visibility: CategoryVisibility,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    let visibility_str = match visibility {
        CategoryVisibility::Public => "public".to_string(),
        CategoryVisibility::Private => "private".to_string(),
    };
    do_add_and_subscribe_category(
        ctx,
        subscriber_account_id,
        subscriber_email,
        name,
        email_address,
        description,
        visibility_str,
        None,
        false,
    )
}

/// Core deactivation logic shared by manual removal, sync-driven automatic unsubscription, and
/// mail-link unsubscription. Sync-driven removal applies to automatic and required
/// subscriptions, but never overrides a manual or link-unsubscribed state. Returns whether the
/// update was actually applied.
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
/// category identified by `category_email_address`, if both exist and the
/// subscription is currently active. No-op if the category or subscription is
/// missing, already inactive, or currently in a manually-managed / link-unsubscribed status
/// (those are never touched by the sync path). Required subscriptions are removed when Django
/// removes the corresponding Verteilpunkt assignment.
pub(crate) fn do_remove_subscription_for_category_email(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    category_email_address: &str,
) -> Result<(), String> {
    let Some(category) = ctx
        .db
        .message_categories()
        .email_address()
        .find(&category_email_address.to_string())
    else {
        return Ok(());
    };

    let Some(sub) = ctx
        .db
        .subscriptions()
        .subscriber_account_id()
        .filter(&subscriber_account_id)
        .find(|s| s.category_id == category.id)
    else {
        return Ok(());
    };

    if sub.status.is_active() {
        let sub_id = sub.id;
        if do_deactivate_subscription(ctx, sub, SubscriptionStatus::AutomaticallyUnsubscribed) {
            log::info!(
                "Deactivated subscription {} for account {} (category email: {})",
                sub_id,
                subscriber_account_id,
                category_email_address
            );
        } else {
            log::info!(
                "Skipped sync-driven unsubscribe of subscription {} for account {} (category email: {}): manually managed",
                sub_id,
                subscriber_account_id,
                category_email_address
            );
        }
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

    if !is_admin && !is_self {
        return Err(
            "Unauthorized: can only remove your own subscriptions or requires admin".to_string(),
        );
    }

    if !is_admin && sub.status == SubscriptionStatus::RequiredSubscribed {
        return Err(
            "Required subscriptions can only be removed by an administrator or Django sync"
                .to_string(),
        );
    }

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

// Procedure: Provision a Stalwart mailbox via JMAP and insert the message category on success.
#[spacetimedb::procedure]
pub fn provision_message_category(
    ctx: &mut spacetimedb::ProcedureContext,
    name: String,
    base: String,
    domain_id: String,
    description: String,
    visibility: CategoryVisibility,
) -> Result<(), String> {
    info!(
        "Provisioning a new Category: name={}, base={}, domain_id={}, description={}",
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

    // 3) Ensure category doesn't already exist
    let exists: bool = ctx.with_tx(|tx| {
        tx.db
            .message_categories()
            .email_address()
            .find(&email_address)
            .is_some()
    });

    if exists {
        error!("The category with that mailadress already exists");
        return Err(format!(
            "Category with email {} already exists",
            email_address
        ));
    }

    // 4) Create the Stalwart mailbox account
    let create_map = serde_json::json!({
        "create": {
            "create-1": {
                "@type": "User",
                "name": base.trim(),
                "description": name.trim(),
                "domainId": domain_id,
                "roles": {
                  "@type": "User"
                },
                "permissions": {
                  "@type": "Inherit"
                },
                "aliases": {},
                "memberGroupIds": {},
                "quotas": {},
                "credentials": {},
                "encryptionAtRest": {
                  "@type": "Disabled"
                }
            }
        }
    });

    let account_payload = serde_json::json!({
        "using": [
            "urn:ietf:params:jmap:core",
            "urn:stalwart:jmap"
        ],
        "methodCalls": [
            ["x:Account/set", create_map, "call-id-1"]
        ]
    });

    let account_res = send_stalwart_jmap_request(ctx, account_payload)?;
    let account_result = jmap_method_result_by_name(&account_res, "x:Account/set")?;
    jmap_check_not_created(account_result, "x:Account/set")?;

    let account_id = account_result
        .get("created")
        .and_then(|created| created.get("create-1"))
        .and_then(jmap_created_id)
        .ok_or_else(|| {
            format!(
                "Missing created account id in JMAP response: {}",
                account_res
            )
        })?;

    // 5) Create an app password for SMTP submission from this category mailbox
    let app_password_description = format!("kommunikationszentrum sender ({email_address})");
    let (stalwart_id, secret) =
        provision_stalwart_app_password(ctx, &account_id, &app_password_description)?;

    // 6) Persist the category and its app password
    ctx.with_tx(|tx| {
        let app_password = tx.db.category_app_passwords().insert(CategoryAppPassword {
            id: 0,
            secret: secret.clone(),
            stalwart_id: stalwart_id.clone(),
            created_at: tx.timestamp,
        });
        tx.db.message_categories().insert(MessageCategory {
            id: 0,
            name: name.clone(),
            email_address: email_address.clone(),
            description: description.clone(),
            active: true,
            visibility,
            app_password_id: Some(app_password.id),
        });
    });

    Ok(())
}
