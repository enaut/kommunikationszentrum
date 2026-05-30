use spacetimedb::{ReducerContext, Table, Timestamp, ViewContext};

use crate::account::{account, account__view, admin_identities__view, is_admin_user, Account};

#[spacetimedb::table(accessor = message_categories, public)]
pub struct MessageCategory {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub name: String,
    #[unique]
    pub email_address: String,
    pub description: String,
    pub active: bool,
}

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
    pub active: bool,
}

/// Returns all subscriptions for admins; only the caller's own subscriptions for regular users.
/// Clients subscribe to this view instead of the raw `subscriptions` table.
#[spacetimedb::view(accessor = visible_subscriptions, public)]
pub fn visible_subscriptions(ctx: &ViewContext) -> Vec<Subscription> {
    let sender = ctx.sender();
    let is_admin = ctx.db.admin_identities().identity().find(&sender).is_some();
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

    let timestamp = ctx.timestamp;

    ctx.db.subscriptions().insert(Subscription {
        id: 0,
        subscriber_account_id,
        subscriber_email,
        category_id,
        subscribed_at: timestamp,
        active: true,
    });
    log::info!(
        "Added subscription for account {} (by identity: {:?})",
        subscriber_account_id,
        ctx.sender()
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn remove_subscription(ctx: &ReducerContext, subscription_id: u64) -> Result<(), String> {
    let sub = ctx
        .db
        .subscriptions()
        .id()
        .find(&subscription_id)
        .ok_or_else(|| format!("Subscription {} not found", subscription_id))?;

    let is_admin = is_admin_user(ctx);
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

    ctx.db.subscriptions().id().delete(&subscription_id);
    log::info!(
        "Removed subscription {} (by identity: {:?})",
        subscription_id,
        ctx.sender()
    );
    Ok(())
}
