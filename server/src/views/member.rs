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

pub fn get_account_config(ctx: &ViewContext) -> AccountConfig {
    let sender = ctx.sender();
    ctx.db
        .account()
        .identity()
        .find(&sender)
        .and_then(|acc| ctx.db.account_configs().account_id().find(&acc.id))
        .unwrap_or_else(|| AccountConfig {
            account_id: 0,
            message_offset: 0,
            message_limit: 50,
            selected_message_category: None,
            member_offset: 0,
            member_limit: 50,
            member_search_query: None,
            viewing_category_id: None,
            language: None,
            theme: None,
            search_matching_accounts: 0,
        })
}

pub fn get_paginated_account_ids(ctx: &ViewContext) -> Vec<u64> {
    if !is_admin_user(ctx) {
        return vec![];
    }
    let config = get_account_config(ctx);
    let mut accounts: Vec<Account> = ctx
        .db
        .account()
        .last_synced()
        .filter(Timestamp::UNIX_EPOCH..)
        .collect();

    if let Some(query) = config.member_search_query {
        let q = query.to_lowercase();
        accounts.retain(|acc| {
            acc.id.to_string().contains(&q) || acc.name.to_lowercase().contains(&q)
            // Note: In a real search we might also join emails, but for the helper
            // simple ID/name match is a good start.
        });
    }

    // Sort by id for deterministic pagination
    accounts.sort_by_key(|a| a.id);

    accounts
        .into_iter()
        .skip(config.member_offset as usize)
        .take(config.member_limit as usize)
        .map(|a| a.id)
        .collect()
}

pub fn get_paginated_received_messages(ctx: &ViewContext) -> Vec<ReceivedMessage> {
    let sender = ctx.sender();
    let is_admin = is_admin_user(ctx);
    let config = get_account_config(ctx);

    let mut messages = if is_admin {
        if let Some(cat_id) = config.selected_message_category {
            ctx.db.received_message().category_id().filter(&cat_id).collect::<Vec<_>>()
        } else {
            ctx.db.received_message().received_at().filter(Timestamp::UNIX_EPOCH..).collect::<Vec<_>>()
        }
    } else {
        match ctx.db.account().identity().find(&sender) {
            Some(acc) => {
                let mut subscribed_category_ids: Vec<u64> = ctx
                    .db
                    .subscriptions()
                    .subscriber_account_id()
                    .filter(&acc.id)
                    .filter(|s| s.status.is_active())
                    .map(|s| s.category_id)
                    .collect();
                
                if let Some(cat_id) = config.selected_message_category {
                    if subscribed_category_ids.contains(&cat_id) {
                        subscribed_category_ids = vec![cat_id];
                    } else {
                        subscribed_category_ids = vec![];
                    }
                }

                subscribed_category_ids.sort_unstable();
                subscribed_category_ids.dedup();
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
    };

    messages.sort_unstable_by(|a, b| b.received_at.cmp(&a.received_at));

    messages
        .into_iter()
        .skip(config.message_offset as usize)
        .take(config.message_limit as usize)
        .collect()
}

#[spacetimedb::view(accessor = visible_account_configs, public)]
pub fn visible_account_configs(ctx: &ViewContext) -> Vec<AccountConfig> {
    let sender = ctx.sender();
    if let Some(acc) = ctx.db.account().identity().find(&sender) {
        if let Some(cfg) = ctx.db.account_configs().account_id().find(&acc.id) {
            return vec![cfg];
        }
    }
    vec![]
}

#[spacetimedb::view(accessor = visible_accounts, public)]
pub fn visible_accounts(ctx: &ViewContext) -> Vec<Account> {
    let sender = ctx.sender();
    let is_admin = is_admin_user(ctx);
    
    let mut account_ids = std::collections::HashSet::new();
    let my_account_id = ctx.db.account().identity().find(&sender).map(|a| a.id);
    
    if let Some(id) = my_account_id {
        account_ids.insert(id);
    }
    
    if is_admin {
        for id in get_paginated_account_ids(ctx) {
            account_ids.insert(id);
        }
        
        let config = get_account_config(ctx);
        if let Some(cat_id) = config.viewing_category_id {
            for sub in ctx.db.subscriptions().category_id().filter(&cat_id) {
                account_ids.insert(sub.subscriber_account_id);
            }
        }
    }
    
    let mut ids_vec: Vec<_> = account_ids.into_iter().collect();
    ids_vec.sort_unstable();
    ids_vec.into_iter().filter_map(|id| ctx.db.account().id().find(&id)).collect()
}

#[spacetimedb::view(accessor = visible_account_emails, public)]
pub fn visible_account_emails(ctx: &ViewContext) -> Vec<AccountEmail> {
    let accounts = visible_accounts(ctx);
    accounts
        .into_iter()
        .flat_map(|acc| ctx.db.account_emails().account_id().filter(&acc.id).collect::<Vec<_>>())
        .collect()
}

#[spacetimedb::view(accessor = visible_subscriptions, public)]
pub fn visible_subscriptions(ctx: &ViewContext) -> Vec<Subscription> {
    let accounts = visible_accounts(ctx);
    accounts
        .into_iter()
        .flat_map(|acc| ctx.db.subscriptions().subscriber_account_id().filter(&acc.id).collect::<Vec<_>>())
        .collect()
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
    get_paginated_received_messages(ctx)
}
