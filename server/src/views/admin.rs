use crate::common::auth::is_admin_user;
use crate::models::account::*;
use crate::models::category::*;
use crate::models::config::*;
use crate::models::delivery::*;
use crate::models::mail_message::*;
use log::info;
use spacetimedb::{Query, ViewContext};

#[spacetimedb::view(accessor = admin_stalwart_config, public)]
pub fn admin_stalwart_config(ctx: &ViewContext) -> impl Query<StalwartConfig> {
    let is_admin = is_admin_user(ctx);
    ctx.from.stalwart_config().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = sender_mail_ingress, public)]
pub fn sender_mail_ingress(ctx: &ViewContext) -> impl Query<MailIngress> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_ingress().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = sender_mail_delivery_messages, public)]
pub fn sender_mail_delivery_messages(ctx: &ViewContext) -> impl Query<MailDeliveryMessage> {
    let is_admin = is_admin_user(ctx);
    ctx.from
        .mail_delivery_message()
        .r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = sender_mail_delivery_pending, public)]
pub fn sender_mail_delivery_pending(ctx: &ViewContext) -> impl Query<MailDeliveryPending> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_delivery_pending().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = sender_mail_delivery_claimed, public)]
pub fn sender_mail_delivery_claimed(ctx: &ViewContext) -> impl Query<MailDeliveryClaimed> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_delivery_claimed().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = sender_mail_delivery_done, public)]
pub fn sender_mail_delivery_done(ctx: &ViewContext) -> impl Query<MailDeliveryDone> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_delivery_done().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = sender_mail_delivery_events, public)]
pub fn sender_mail_delivery_events(ctx: &ViewContext) -> impl Query<MailDeliveryEvent> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_delivery_events().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = sender_mail_delivery_temporary_failed, public)]
pub fn sender_mail_delivery_temporary_failed(
    ctx: &ViewContext,
) -> impl Query<MailDeliveryTemporaryFailed> {
    let is_admin = is_admin_user(ctx);
    ctx.from
        .mail_delivery_temporary_failed()
        .r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = sender_mail_messages, public)]
pub fn sender_mail_messages(ctx: &ViewContext) -> impl Query<MailMessage> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_message().r#filter(move |_| is_admin)
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

#[spacetimedb::view(accessor = visible_webhook_tokens, public)]
pub fn visible_webhook_tokens(ctx: &ViewContext) -> impl Query<WebhookToken> {
    let is_admin = is_admin_user(ctx);
    ctx.from.webhook_tokens().r#filter(move |_| is_admin)
}

#[spacetimedb::view(accessor = visible_category_app_passwords, public)]
pub fn visible_category_app_passwords(ctx: &ViewContext) -> impl Query<CategoryAppPassword> {
    let is_admin = is_admin_user(ctx);
    ctx.from
        .category_app_passwords()
        .r#filter(move |_| is_admin)
}
