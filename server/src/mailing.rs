use spacetimedb::{ReducerContext, Table, Timestamp};

use crate::account::{account, is_admin_user, Account};

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
}

#[spacetimedb::table(accessor = subscriptions)]
pub struct Subscription {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub subscriber_account_id: u64,
    #[index(btree)]
    pub subscriber_email: String,
    #[index(btree)]
    pub category_id: u64,
    pub subscribed_at: Timestamp,
    pub active: bool,
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
