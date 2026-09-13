use spacetimedb::{AnonymousViewContext, SpacetimeType, ViewContext};

use crate::models::account::account__view;
use crate::models::mta::received_message__view;
use crate::models::topic::subscriptions__view;

#[derive(SpacetimeType, Clone, Debug)]
pub struct CountRow {
    pub count: u64,
}

#[derive(SpacetimeType, Clone, Debug)]
pub struct TopicMessageCount {
    pub topic_id: u64,
    pub count: u64,
}

#[derive(SpacetimeType, Clone, Debug)]
pub struct TopicSubscriberCount {
    pub topic_id: u64,
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

#[spacetimedb::view(accessor = topic_message_counts, public)]
pub fn topic_message_counts(ctx: &ViewContext) -> Vec<TopicMessageCount> {
    let mut counts = Vec::new();
    for topic in crate::views::member::visible_message_topics(ctx) {
        let count = ctx.db.received_message().topic_id().filter(&topic.id).count() as u64;
        counts.push(TopicMessageCount {
            topic_id: topic.id,
            count,
        });
    }
    counts
}

#[spacetimedb::view(accessor = topic_subscriber_counts, public)]
pub fn topic_subscriber_counts(ctx: &ViewContext) -> Vec<TopicSubscriberCount> {
    let mut counts = Vec::new();
    for topic in crate::views::member::visible_message_topics(ctx) {
        let count = ctx.db.subscriptions().topic_id().filter(&topic.id)
            .filter(|s| s.status.is_active())
            .count() as u64;
        counts.push(TopicSubscriberCount {
            topic_id: topic.id,
            count,
        });
    }
    counts
}
