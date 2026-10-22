use std::collections::HashSet;

use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::SubscriptionStatus;
use crate::module_bindings::{
    MessageTopic, TopicVisibility,
    dioxus::{
        use_reducer_add_subscription, use_reducer_remove_subscription, use_subscription,
        use_table_visible_categories, use_table_visible_message_topic_categories,
        use_table_visible_message_topics, use_table_visible_subscriptions,
    },
};
use crate::oauth::UserInfo;
use crate::router::ActiveView;

/// Tab identity for the member subscriptions page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CategoryTab {
    Category(u64),
    Sonstige,
}

/// Default view for all users: lists all active message topics and lets the
/// user subscribe or unsubscribe with a single button click. Topics are
/// grouped into tabs by category; topics without categories appear under "Sonstige".
#[component]
pub fn SubscriptionsPage(user_info: UserInfo, active_view: Signal<ActiveView>) -> Element {
    use_subscription(&[
        "SELECT * FROM visible_accounts",
        "SELECT * FROM context_accounts",
        "SELECT * FROM visible_account_emails",
        "SELECT * FROM visible_message_topics",
        "SELECT * FROM visible_subscriptions",
        "SELECT * FROM visible_categories",
        "SELECT * FROM visible_message_topic_categories",
    ]);
    let accounts = crate::module_bindings::dioxus::use_table_visible_accounts();
    let context_accounts = crate::module_bindings::dioxus::use_table_context_accounts();
    let account_emails = crate::module_bindings::dioxus::use_table_visible_account_emails();
    let topics = use_table_visible_message_topics();
    let subscriptions = use_table_visible_subscriptions();
    let categories = use_table_visible_categories();
    let topic_categories = use_table_visible_message_topic_categories();
    let add_subscription = use_reducer_add_subscription();
    let remove_subscription = use_reducer_remove_subscription();

    let my_account = context_accounts()
        .into_iter()
        .find(|a| a.external_id == user_info.subject_id)
        .or_else(|| {
            accounts()
                .into_iter()
                .find(|a| a.external_id == user_info.subject_id)
        });
    let account_id = my_account.as_ref().map(|a| a.id).unwrap_or(0);
    let my_primary_email_id = my_account.map(|a| a.primary_email_id).unwrap_or(0);
    let my_emails: Vec<_> = account_emails()
        .into_iter()
        .filter(|e| e.account_id == account_id)
        .collect();

    let mut active_tab = use_signal(|| CategoryTab::Sonstige);
    let mut user_picked_tab = use_signal(|| false);

    let active_topics: Vec<MessageTopic> = topics().into_iter().filter(|t| t.active).collect();

    let category_ids: Vec<u64> = {
        let mut rows = categories();
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        rows.into_iter().map(|c| c.id).collect()
    };

    let links = topic_categories();
    let categorized_topic_ids: HashSet<u64> = links.iter().map(|l| l.topic_id).collect();

    let sonstige_topics: Vec<MessageTopic> = active_topics
        .iter()
        .filter(|t| !categorized_topic_ids.contains(&t.id))
        .cloned()
        .collect();

    let show_sonstige = !sonstige_topics.is_empty() || category_ids.is_empty();

    // Keep the default tab in sync with loaded data until the user picks one.
    use_effect(move || {
        if user_picked_tab() {
            return;
        }
        let mut categories_sorted = categories();
        categories_sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        if let Some(first) = categories_sorted.first() {
            active_tab.set(CategoryTab::Category(first.id));
        } else {
            active_tab.set(CategoryTab::Sonstige);
        }
    });

    let current = active_tab();
    let visible_topics: Vec<MessageTopic> = match current {
        CategoryTab::Sonstige => sonstige_topics.clone(),
        CategoryTab::Category(cat_id) => {
            let topic_ids: HashSet<u64> = links
                .iter()
                .filter(|l| l.category_id == cat_id)
                .map(|l| l.topic_id)
                .collect();
            active_topics
                .into_iter()
                .filter(|t| topic_ids.contains(&t.id))
                .collect()
        }
    };

    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3 align-items-center justify-content-between",
                Col { md: ColumnSize::Auto,
                    h2 { class: "mb-0",
                        Icon { name: "envelope-check", class: "me-2" }
                        {tid!("subscriptions-page-title")}
                    }
                    p { class: "text-muted mt-1 mb-0",
                        {tid!("subscriptions-page-description")}
                    }
                }
                Col { md: ColumnSize::Auto, class: "mt-2 mt-md-0",
                    div { class: "d-flex justify-content-between align-items-center gap-3 p-2 px-3 bg-light rounded border",
                        span { class: "text-muted small d-flex align-items-center",
                            Icon { name: "envelope-plus", class: "me-2 fs-3 lh-1" }
                            {tid!("subscriptions-manage-emails-hint")}
                        }
                        Button {
                            color: Color::Primary,
                            outline: true,
                            size: Size::Sm,
                            class: "text-nowrap",
                            onclick: move |_| active_view.set(ActiveView::Profile),
                            Icon { name: "person", class: "me-1" }
                            {tid!("subscriptions-manage-emails-link")}
                        }
                    }
                }
            }

            if !category_ids.is_empty() || show_sonstige {
                Nav {
                    tabs: true,
                    class: "mb-3",
                    for cat_id in category_ids.iter().copied() {
                        CategoryTabButton {
                            key: "{cat_id}",
                            category_id: cat_id,
                            active: current == CategoryTab::Category(cat_id),
                            on_select: move |_| {
                                user_picked_tab.set(true);
                                active_tab.set(CategoryTab::Category(cat_id));
                            },
                        }
                    }
                    if show_sonstige {
                        NavItem {
                            NavLink {
                                active: current == CategoryTab::Sonstige,
                                prevent_default: true,
                                onclick: move |_| {
                                    user_picked_tab.set(true);
                                    active_tab.set(CategoryTab::Sonstige);
                                },
                                {tid!("subscriptions-tab-other")}
                            }
                        }
                    }
                }
            }

            if visible_topics.is_empty() {
                Alert { color: Color::Info,
                    Icon { name: "info-circle", class: "me-2" }
                    if category_ids.is_empty() && sonstige_topics.is_empty() {
                        {tid!("subscriptions-empty")}
                    } else {
                        {tid!("subscriptions-empty-category")}
                    }
                }
            } else {
                Row {
                    for top in visible_topics {
                        {
                            let top_subscriptions: Vec<_> = subscriptions().into_iter().filter(|s| {
                                s.topic_id == top.id
                                    && s.subscriber_account_id == account_id
                                    && crate::pages::is_active_subscription(&s.status)
                            }).collect();
                            let is_subscribed_any = !top_subscriptions.is_empty();
                            let topic_id = top.id;
                            let add = add_subscription.clone();
                            let remove = remove_subscription.clone();
                            let my_emails_clone = my_emails.clone();

                            rsx! {
                                Col { md: ColumnSize::Span(6), lg: ColumnSize::Span(4), class: "mb-3",
                                    Card {
                                        class: if is_subscribed_any { "h-100 border-dark bg-light" } else { "h-100 border-light" },
                                        body_class: "d-flex flex-column",
                                        header: rsx! {
                                            h5 { class: "card-title mb-0", "{top.name}" }
                                            if is_subscribed_any {
                                                Badge { color: Color::Success, class: "ms-2", {tid!("subscriptions-subscribed")} }
                                            }

                                            if top.visibility == TopicVisibility::Public {
                                                Badge { color: Color::Info, class: "ms-2 align-middle", {tid!("subscriptions-public")} }
                                            } else {
                                                Badge { color: Color::Warning, class: "ms-2 align-middle", {tid!("subscriptions-private")} }
                                            }
                                        },
                                        body: rsx! {
                                            p { class: "card-text text-muted small flex-grow-1", "{top.description}" }
                                            p { class: "card-text mb-3",
                                                small { class: "text-muted",
                                                    Icon { name: "envelope", class: "me-1" }
                                                    "{top.email_address}"
                                                }
                                            }
                                            div { class: "mt-auto pt-2 border-top",
                                                small { class: "text-muted fw-bold d-block mb-2", {tid!("subscriptions-card-email-subscriptions")} }
                                                div { class: "d-flex flex-column gap-2",
                                                    for email in my_emails_clone {
                                                        {
                                                            let email_id = email.id;
                                                            let sub_for_email = top_subscriptions.iter().find(|s| s.account_email_id == email_id);
                                                            let is_subbed = sub_for_email.is_some();
                                                            let is_required = sub_for_email.is_some_and(|s| matches!(s.status, SubscriptionStatus::RequiredSubscribed));
                                                            let sub_id = sub_for_email.map(|s| s.id);
                                                            let add_fn = add.clone();
                                                            let remove_fn = remove.clone();
                                                            let input_id = format!("sub-check-{topic_id}-{email_id}");

                                                            rsx! {
                                                                div { class: "form-check",
                                                                    input {
                                                                        class: "form-check-input",
                                                                        r#type: "checkbox",
                                                                        id: "{input_id}",
                                                                        checked: is_subbed,
                                                                        disabled: is_required || !email.is_verified,
                                                                        onchange: move |_| {
                                                                            if is_subbed {
                                                                                if let Some(id) = sub_id {
                                                                                    info!("Unsubscribing {email_id} from topic {topic_id}");
                                                                                    if let Err(err) = remove_fn(id) {
                                                                                        error!("remove_subscription failed: {err:?}");
                                                                                    }
                                                                                }
                                                                            } else {
                                                                                info!("Subscribing {email_id} to topic {topic_id}");
                                                                                if let Err(err) = add_fn(account_id, email_id, topic_id) {
                                                                                    error!("add_subscription failed: {err:?}");
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    label {
                                                                        class: if !email.is_verified {
                                                                            "form-check-label small d-flex align-items-center flex-wrap gap-1 text-muted"
                                                                        } else {
                                                                            "form-check-label small d-flex align-items-center flex-wrap gap-1"
                                                                        },
                                                                        r#for: "{input_id}",
                                                                        if email.id == my_primary_email_id {
                                                                            strong { "{email.email} " }
                                                                            span { class: "text-muted", {tid!("subscriptions-primary-badge")} }
                                                                        } else {
                                                                            span { "{email.email}" }
                                                                        }
                                                                        if !email.is_verified {
                                                                            span {
                                                                                title: tid!("subscriptions-email-unconfirmed-tooltip"),
                                                                                Badge { color: Color::Secondary, class: "ms-1", {tid!("subscriptions-email-not-confirmed")} }
                                                                            }
                                                                        }
                                                                        if is_required {
                                                                            Badge { color: Color::Info, class: "ms-1", {tid!("subscriptions-required")} }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Tab button that always reads the category name from the live `visible_categories` signal.
#[component]
fn CategoryTabButton(category_id: u64, active: bool, on_select: EventHandler<()>) -> Element {
    let categories = use_table_visible_categories();
    let name = use_memo(move || {
        categories()
            .into_iter()
            .find(|c| c.id == category_id)
            .map(|c| c.name)
            .unwrap_or_default()
    });
    let label = name();
    if label.is_empty() {
        return rsx! {};
    }

    rsx! {
        NavItem {
            NavLink {
                active,
                prevent_default: true,
                onclick: move |_| on_select.call(()),
                "{label}"
            }
        }
    }
}
