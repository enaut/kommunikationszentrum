use crate::common::auth::is_admin_user;
use crate::models::account::*;
use crate::models::category::*;
use crate::models::domain::*;
use crate::models::mta::*;
use spacetimedb::{Query, Timestamp, ViewContext};

#[spacetimedb::view(accessor = visible_domains, public)]
pub fn visible_domains(ctx: &ViewContext) -> impl Query<Domain> {
    let is_admin = is_admin_user(ctx);
    ctx.from.domains().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = visible_accounts, public)]
pub fn visible_accounts(ctx: &ViewContext) -> Vec<Account> {
    let sender = ctx.sender();
    let is_admin = is_admin_user(ctx);
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

#[spacetimedb::view(accessor = visible_message_categories, public)]
pub fn visible_message_categories(ctx: &ViewContext) -> Vec<MessageCategory> {
    let sender = ctx.sender();
    let is_admin = is_admin_user(ctx);

    let all_categories = ctx.db.message_categories().visibility();

    let public_categories = all_categories
        .filter(CategoryVisibility::Public)
        .collect::<Vec<_>>();

    if is_admin {
        // Admins see all categories
        let private_categories = all_categories
            .filter(CategoryVisibility::Private)
            .collect::<Vec<_>>();
        let mut result = public_categories;
        result.extend(private_categories);
        return result;
    }

    let has_account = ctx.db.account().identity().find(&sender).is_some();
    if !has_account {
        return vec![];
    }

    // For regular users: show public categories + private categories they're subscribed to
    let account = ctx
        .db
        .account()
        .identity()
        .find(&sender)
        .expect("Account must exist");
    let subscribed_category_ids: Vec<u64> = ctx
        .db
        .subscriptions()
        .subscriber_account_id()
        .filter(&account.id)
        .map(|sub| sub.category_id)
        .collect();

    let mut result = public_categories;
    let private_categories: Vec<MessageCategory> = all_categories
        .filter(CategoryVisibility::Private)
        .filter(|cat| subscribed_category_ids.contains(&cat.id))
        .collect();
    result.extend(private_categories);

    result
}

#[spacetimedb::view(accessor = visible_topics, public)]
pub fn visible_topics(ctx: &ViewContext) -> impl Query<Topic> {
    ctx.from.topics()
}

#[spacetimedb::view(accessor = visible_message_category_topics, public)]
pub fn visible_message_category_topics(ctx: &ViewContext) -> impl Query<MessageCategoryTopic> {
    ctx.from.message_category_topics()
}

#[spacetimedb::view(accessor = visible_messages, public)]
pub fn visible_messages(ctx: &ViewContext) -> Vec<ReceivedMessage> {
    let sender = ctx.sender();
    let is_admin = is_admin_user(ctx);
    if is_admin {
        ctx.db
            .received_message()
            .received_at()
            .filter(Timestamp::UNIX_EPOCH..)
            .collect()
    } else {
        match ctx.db.account().identity().find(&sender) {
            Some(acc) => {
                let subscribed_category_ids: Vec<u64> = ctx
                    .db
                    .subscriptions()
                    .subscriber_account_id()
                    .filter(&acc.id)
                    .filter(|s| s.status.is_active())
                    .map(|s| s.category_id)
                    .collect();
                subscribed_category_ids
                    .into_iter()
                    .flat_map(|cat_id| {
                        ctx.db
                            .received_message()
                            .category_id()
                            .filter(&cat_id)
                            .collect::<Vec<_>>()
                    })
                    .collect()
            }
            None => vec![],
        }
    }
}
