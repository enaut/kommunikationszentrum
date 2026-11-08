use crate::common::auth::is_admin_user;
use crate::models::account::*;
use crate::models::config::*;

use crate::models::sender::*;
use crate::models::topic::*;
use log::info;
use spacetimedb::{Query, ViewContext};

#[spacetimedb::view(accessor = admin_stalwart_config, public)]
pub fn admin_stalwart_config(ctx: &ViewContext) -> impl Query<StalwartConfig> {
    let is_admin = is_admin_user(ctx);
    ctx.from.stalwart_config().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = admin_mail_retention_config, public)]
pub fn admin_mail_retention_config(ctx: &ViewContext) -> impl Query<MailRetentionConfig> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_retention_config().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = sender_jmap_mail_deliveries, public)]
pub fn sender_jmap_mail_deliveries(ctx: &ViewContext) -> impl Query<JmapMailDelivery> {
    let is_admin = is_admin_user(ctx);
    ctx.from.jmap_mail_deliveries().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = sender_jmap_mail_delivery_events, public)]
pub fn sender_jmap_mail_delivery_events(ctx: &ViewContext) -> impl Query<JmapMailDeliveryEvent> {
    let is_admin = is_admin_user(ctx);
    ctx.from
        .jmap_mail_delivery_events()
        .r#filter(move |_| is_admin)
}

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

#[spacetimedb::view(accessor = active_unsubscribe_tokens, public)]
pub fn active_unsubscribe_tokens(ctx: &ViewContext) -> impl Query<SubscriptionUnsubscribeToken> {
    let is_admin = is_admin_user(ctx);
    ctx.from
        .subscription_unsubscribe_tokens()
        .r#where(move |t| t.active.eq(true).and(t.active.eq(is_admin)))
}

#[spacetimedb::view(accessor = visible_admin_identities, public)]
pub fn visible_admin_identities(ctx: &ViewContext) -> impl Query<AdminIdentity> {
    info!("Checking if user is admin for visible_admin_identities view");
    let is_admin = is_admin_user(ctx);
    ctx.from.admin_identities().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = admin_accounts, public)]
pub fn admin_accounts(ctx: &ViewContext) -> impl Query<Account> {
    let is_admin = is_admin_user(ctx);
    ctx.from
        .account()
        .left_semijoin(ctx.from.admin_identities(), |account, admin| {
            account.identity.eq(admin.identity)
        })
        .filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = visible_webhook_tokens, public)]
pub fn visible_webhook_tokens(ctx: &ViewContext) -> impl Query<WebhookToken> {
    let is_admin = is_admin_user(ctx);
    ctx.from.webhook_tokens().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = visible_topic_app_passwords, public)]
pub fn visible_topic_app_passwords(ctx: &ViewContext) -> impl Query<TopicAppPassword> {
    let is_admin = is_admin_user(ctx);
    ctx.from.topic_app_passwords().r#filter(move |_| is_admin)
}
