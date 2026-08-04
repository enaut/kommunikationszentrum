use log::{error, info};
use serde::{Deserialize, Serialize};
use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp, ViewContext};

use crate::account::{account, account__view, is_admin_identity, is_admin_user, Account};

// Private: clients never subscribe to this table directly. `visible_message_categories`
// below is the only way clients can read category rows.
#[spacetimedb::table(accessor = message_categories)]
pub struct MessageCategory {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub name: String,
    #[unique]
    pub email_address: String,
    pub description: String,
    pub active: bool,
    /// Controls whether regular members can see this category. Admins can see
    /// both variants.
    #[index(btree)]
    #[default(CategoryVisibility::Public)]
    pub visibility: CategoryVisibility,
}

/// Determines who can discover a message category in the member-facing view.
#[derive(SpacetimeType, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CategoryVisibility {
    #[default]
    Public,
    Private,
}

impl CategoryVisibility {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "public" | "Public" => Ok(Self::Public),
            "private" | "Private" => Ok(Self::Private),
            _ => Err(format!(
                "Invalid category visibility '{value}'; expected 'public' or 'private'"
            )),
        }
    }
}

/// A reusable category tag, such as `verteilpunkt` or `arbeitsgruppe`.
#[spacetimedb::table(accessor = topics)]
pub struct Topic {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub name: String,
}

/// Many-to-many assignment of topics to message categories.
#[spacetimedb::table(accessor = message_category_topics)]
pub struct MessageCategoryTopic {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub category_id: u64,
    #[index(btree)]
    pub topic_id: u64,
}

/// Category data as sent by the Django user-sync webhook for a single
/// mailing-list assignment (e.g. a Verteilpunkt). Used to ensure the
/// category exists and to subscribe an account to it in one step.
#[derive(Clone, Serialize, Deserialize)]
pub struct CategorySyncData {
    pub name: String,
    pub email_address: String,
    pub description: String,
    #[serde(default = "default_category_visibility")]
    pub visibility: String,
    /// `None` leaves an existing category's topics unchanged. An empty array
    /// explicitly removes all of its topic assignments.
    #[serde(default)]
    pub topics: Option<Vec<String>>,
    #[serde(default)]
    pub required: bool,
}

fn default_category_visibility() -> String {
    "public".to_string()
}

/// Lifecycle status of a [`Subscription`]. The Django sync path controls automatic and required
/// subscriptions. `ManuallySubscribed` / `ManuallyUnsubscribed` are set by an explicit admin or
/// member action, while `LinkUnsubscribed` is set by a one-click `List-Unsubscribe` request.
/// Required subscriptions are managed by Django and cannot be removed by the member.
#[derive(SpacetimeType, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionStatus {
    AutomaticallySubscribed,
    AutomaticallyUnsubscribed,
    ManuallySubscribed,
    ManuallyUnsubscribed,
    LinkUnsubscribed,
    RequiredSubscribed,
}

impl SubscriptionStatus {
    /// Whether a subscription with this status should currently receive mail.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::AutomaticallySubscribed | Self::ManuallySubscribed | Self::RequiredSubscribed
        )
    }

    /// Whether this status was set by the automatic sync path, as opposed to a manual
    /// admin/member action or a mail-link unsubscribe. Only subscriptions currently in an
    /// automatic status may be overwritten by the sync path.
    fn is_automatic(&self) -> bool {
        matches!(
            self,
            Self::AutomaticallySubscribed | Self::AutomaticallyUnsubscribed
        )
    }

    /// Whether this state is controlled by the Django synchronization path.
    fn is_sync_managed(&self) -> bool {
        self.is_automatic() || matches!(self, Self::RequiredSubscribed)
    }
}

// Private: clients never subscribe to this table directly. `visible_subscriptions`
// below is the only way clients can read subscription rows.
#[derive(Clone)]
#[spacetimedb::table(accessor = subscriptions)]
pub struct Subscription {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub subscriber_account_id: u64,
    #[index(btree)]
    pub subscriber_email: String,
    #[index(btree)]
    pub category_id: u64,
    pub subscribed_at: Timestamp,
    #[index(btree)]
    pub status: SubscriptionStatus,
}

// Private: clients never subscribe to this table directly. `active_unsubscribe_tokens`
// below is the only way clients can read unsubscribe-token rows.
#[derive(Clone)]
#[spacetimedb::table(accessor = subscription_unsubscribe_tokens)]
pub struct SubscriptionUnsubscribeToken {
    #[primary_key]
    pub token: String,
    #[unique]
    pub subscription_id: u64,
    #[index(btree)]
    pub created_at: Timestamp,
    pub active: bool,
    pub revoked_at: Timestamp,
}

/// Returns all subscriptions for admins; only the caller's own subscriptions for regular users.
/// Clients subscribe to this view instead of the raw `subscriptions` table.
#[spacetimedb::view(accessor = visible_subscriptions, public)]
pub fn visible_subscriptions(ctx: &ViewContext) -> Vec<Subscription> {
    let sender = ctx.sender();
    let is_admin = is_admin_user(ctx);
    if is_admin {
        ctx.db
            .subscriptions()
            .subscriber_account_id()
            .filter(0u64..)
            .collect()
    } else {
        match ctx.db.account().identity().find(&sender) {
            Some(acc) => ctx
                .db
                .subscriptions()
                .subscriber_account_id()
                .filter(&acc.id)
                .collect(),
            None => vec![],
        }
    }
}

/// Full active-subscription fan-out list, used by the `sender` service to route
/// outgoing mail. Restricted to admins (the `sender` service connects with an
/// admin identity); regular users get an empty list and should use
/// `visible_subscriptions` instead.
#[spacetimedb::view(accessor = active_subscriptions, public)]
pub fn active_subscriptions(ctx: &ViewContext) -> Vec<Subscription> {
    if !is_admin_user(ctx) {
        return vec![];
    }
    // Uses the `status` B-tree index instead of scanning the whole table.
    ctx.db
        .subscriptions()
        .status()
        .filter(SubscriptionStatus::AutomaticallySubscribed)
        .chain(
            ctx.db
                .subscriptions()
                .status()
                .filter(SubscriptionStatus::ManuallySubscribed),
        )
        .chain(
            ctx.db
                .subscriptions()
                .status()
                .filter(SubscriptionStatus::RequiredSubscribed),
        )
        .collect()
}

/// Active unsubscribe tokens. Restricted to admins (the `sender` service needs
/// every subscriber's token to build one-click unsubscribe links); regular
/// users get an empty list.
#[spacetimedb::view(accessor = active_unsubscribe_tokens, public)]
pub fn active_unsubscribe_tokens(ctx: &ViewContext) -> Vec<SubscriptionUnsubscribeToken> {
    if !is_admin_user(ctx) {
        return vec![];
    }
    // Uses the `created_at` B-tree index as a full-range scan, then filters in Rust.
    ctx.db
        .subscription_unsubscribe_tokens()
        .created_at()
        .filter(Timestamp::UNIX_EPOCH..)
        .filter(|token| token.active)
        .collect()
}

/// Returns all message categories once the caller has an associated account
/// (i.e. is a known SoLaWi member) see public categories; admins see both
/// public and private categories. Identities without an account (not yet
/// synced, or never a member) see an empty list. Clients subscribe to this
/// view instead of the raw `message_categories` table.
#[spacetimedb::view(accessor = visible_message_categories, public)]
pub fn visible_message_categories(ctx: &ViewContext) -> Vec<MessageCategory> {
    let sender = ctx.sender();
    let has_account = ctx.db.account().identity().find(&sender).is_some();
    let is_admin = is_admin_user(ctx);
    let categories = ctx.db.message_categories().visibility();
    if is_admin {
        categories
            .filter(CategoryVisibility::Public)
            .chain(categories.filter(CategoryVisibility::Private))
            .collect()
    } else if has_account {
        categories.filter(CategoryVisibility::Public).collect()
    } else {
        vec![]
    }
}

#[spacetimedb::reducer]
pub fn add_message_category(
    ctx: &ReducerContext,
    name: String,
    email_address: String,
    description: String,
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
        visibility: CategoryVisibility::Public,
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
    if ctx
        .db
        .message_categories()
        .id()
        .find(&category_id)
        .is_none()
    {
        return Err(format!("Message category {} not found", category_id));
    }
    ctx.db.message_categories().id().delete(&category_id);
    log::info!(
        "Removed message category {} (by identity: {:?})",
        category_id,
        ctx.sender()
    );
    Ok(())
}

/// Updates the editable fields (name, description) of an existing message
/// category. The `email_address` is immutable via this reducer since it is
/// used to route incoming mail and to match categories during user sync.
#[spacetimedb::reducer]
pub fn update_message_category(
    ctx: &ReducerContext,
    category_id: u64,
    name: String,
    description: String,
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

    let token = upsert_subscription_unsubscribe_token(ctx, subscription.id)?;
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
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    do_add_and_subscribe_category(
        ctx,
        subscriber_account_id,
        subscriber_email,
        name,
        email_address,
        description,
        default_category_visibility(),
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

fn upsert_subscription_unsubscribe_token(
    ctx: &ReducerContext,
    subscription_id: u64,
) -> Result<String, String> {
    if let Some(existing) = ctx
        .db
        .subscription_unsubscribe_tokens()
        .subscription_id()
        .find(&subscription_id)
    {
        if existing.active {
            return Ok(existing.token);
        }

        let mut updated = existing.clone();
        updated.active = true;
        updated.revoked_at = Timestamp::UNIX_EPOCH;
        updated.created_at = ctx.timestamp;
        ctx.db
            .subscription_unsubscribe_tokens()
            .token()
            .update(updated.clone());
        return Ok(updated.token);
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
    Ok(token)
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
pub fn ensure_subscription_unsubscribe_token(
    ctx: &ReducerContext,
    subscription_id: u64,
) -> Result<(), String> {
    upsert_subscription_unsubscribe_token(ctx, subscription_id).map(|_| ())
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
    email_address: String,
    description: String,
) -> Result<(), String> {
    info!(
        "Provisioning a new Category: {}, {}, {}",
        name, email_address, description
    );
    // 1) Authorization check: capture the procedure caller identity and check inside a transaction
    let caller = ctx.sender();
    info!("Checking permissions for identity: {:?}", caller);
    let is_admin: bool = ctx.with_tx(|tx| is_admin_identity(tx, caller));

    if !is_admin {
        return Err("Unauthorized: Admin access required".to_string());
    }

    info!("User has required permissions!");

    // 2) Ensure category doesn't already exist
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

    // 3) Read compile-time configuration for JMAP URL and admin token
    let jmap_base = env!("STALWART_JMAP_URL");
    let admin_token = env!("STALWART_ADMIN_TOKEN");

    let endpoint = if jmap_base.ends_with("/jmap") {
        jmap_base.trim_end_matches('/').to_string()
    } else {
        format!("{}/jmap", jmap_base.trim_end_matches('/'))
    };

    // 4) Build JMAP payload
    let create_map = serde_json::json!({
        "create": {
            "create-1": {
                "@type": "User",
                "name": email_address.split("@").next(),
                "description": name,
                "domainId": "c",
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

    let payload = serde_json::json!({
        "using": [
            "urn:ietf:params:jmap:core",
            "urn:stalwart:jmap"
        ],
        "methodCalls": [
            ["x:Account/set", create_map, "call-id-1"]
        ]
    });

    let body = serde_json::to_vec(&payload).map_err(|e| {
        error!("Failed to serialize JMAP payload: {}", e);
        format!("Failed to serialize JMAP payload: {}", e)
    })?;

    info!("body created!");

    let request = spacetimedb::http::Request::builder()
        .uri(endpoint)
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", admin_token))
        .extension(spacetimedb::http::Timeout(
            spacetimedb::TimeDuration::from_micros(30_000_000),
        ))
        .body(body)
        .map_err(|e| format!("Failed to build HTTP request: {:?}", e))?;
    info!("request created!");
    // 5) Perform HTTP request
    let response = ctx.http.send(request).map_err(|e| {
        error!("Failed to perform request: {}", e);
        format!("HTTP send failed: {:?}", e)
    })?;

    info!("Response: {:?}", response.status());

    let (parts, body) = response.into_parts();

    if parts.status != 200 {
        let body = body.into_string_lossy();
        error!(
            "Stalwart responded with status {} and body {}",
            parts.status, body
        );
        return Err(format!(
            "Stalwart responded with status {} and body {}",
            parts.status, body
        ));
    }

    let body_bytes = body.into_bytes();
    let res_body: serde_json::Value = serde_json::from_slice(&body_bytes)
        .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

    info!("Response Body: {}", res_body);

    // Inspect methodResponses for x:Account/set and check for `notCreated`
    if let Some(method_responses) = res_body.get("methodResponses").and_then(|v| v.as_array()) {
        for entry in method_responses {
            if let Some(method_name) = entry.get(0).and_then(|v| v.as_str()) {
                if method_name == "x:Account/set" {
                    if let Some(result_obj) = entry.get(1) {
                        if let Some(not_created) = result_obj.get("notCreated") {
                            if not_created
                                .as_object()
                                .map(|m| !m.is_empty())
                                .unwrap_or(false)
                            {
                                return Err(format!("JMAP reported notCreated: {}", not_created));
                            }
                        }
                        // Success path: insert the category inside a transaction
                        ctx.with_tx(|tx| {
                            tx.db.message_categories().insert(MessageCategory {
                                id: 0,
                                name: name.clone(),
                                email_address: email_address.clone(),
                                description: description.clone(),
                                active: true,
                                visibility: CategoryVisibility::Public,
                            });
                        });

                        return Ok(());
                    }
                }
            }
        }
    }

    Err(format!("Unexpected JMAP response: {}", res_body))
}
