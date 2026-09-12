use spacetimedb::{AnonymousViewContext, SpacetimeType};

use crate::models::account::account__view;
use crate::models::category::{CategoryVisibility, message_categories__view, subscriptions__view};
use crate::models::mta::received_message__view;

#[derive(SpacetimeType, Clone, Debug)]
pub struct CountRow {
    pub count: u64,
}

#[derive(SpacetimeType, Clone, Debug)]
pub struct CategoryMessageCount {
    pub category_id: u64,
    pub count: u64,
}

#[derive(SpacetimeType, Clone, Debug)]
pub struct CategorySubscriberCount {
    pub category_id: u64,
    pub count: u64,
}

#[spacetimedb::view(accessor = total_accounts, public)]
pub fn total_accounts(ctx: &AnonymousViewContext) -> Vec<CountRow> {
    vec![CountRow {
        count: ctx.db.account().count(),
    }]
}

#[spacetimedb::view(accessor = total_messages, public)]
pub fn total_messages(ctx: &AnonymousViewContext) -> Vec<CountRow> {
    vec![CountRow {
        count: ctx.db.received_message().count(),
    }]
}

#[spacetimedb::view(accessor = category_message_counts, public)]
pub fn category_message_counts(ctx: &AnonymousViewContext) -> Vec<CategoryMessageCount> {
    let mut counts = Vec::new();
    let public_cats = ctx.db.message_categories().visibility().filter(CategoryVisibility::Public);
    let private_cats = ctx.db.message_categories().visibility().filter(CategoryVisibility::Private);
    for cat in public_cats.chain(private_cats) {
        let count = ctx.db.received_message().category_id().filter(&cat.id).count() as u64;
        counts.push(CategoryMessageCount {
            category_id: cat.id,
            count,
        });
    }
    counts
}

#[spacetimedb::view(accessor = category_subscriber_counts, public)]
pub fn category_subscriber_counts(ctx: &AnonymousViewContext) -> Vec<CategorySubscriberCount> {
    let mut counts = Vec::new();
    let public_cats = ctx.db.message_categories().visibility().filter(CategoryVisibility::Public);
    let private_cats = ctx.db.message_categories().visibility().filter(CategoryVisibility::Private);
    for cat in public_cats.chain(private_cats) {
        let count = ctx.db.subscriptions().category_id().filter(&cat.id)
            .filter(|s| s.status.is_active())
            .count() as u64;
        counts.push(CategorySubscriberCount {
            category_id: cat.id,
            count,
        });
    }
    counts
}
