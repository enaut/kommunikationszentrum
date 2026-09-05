use std::collections::HashSet;

use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::SubscriptionStatus;
use crate::module_bindings::{
    dioxus::{
        use_reducer_add_subscription, use_reducer_remove_subscription, use_subscription,
        use_table_visible_message_categories, use_table_visible_message_category_topics,
        use_table_visible_subscriptions, use_table_visible_topics,
    },
    CategoryVisibility, MessageCategory,
};
use crate::oauth::UserInfo;

/// Tab identity for the member subscriptions page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TopicTab {
    Topic(u64),
    Sonstige,
}

/// Default view for all users: lists all active message categories and lets the
/// user subscribe or unsubscribe with a single button click. Categories are
/// grouped into tabs by topic; categories without topics appear under "Sonstige".
#[component]
pub fn SubscriptionsPage(user_info: UserInfo) -> Element {
    use_subscription(&[
        "SELECT * FROM visible_message_categories",
        "SELECT * FROM visible_subscriptions",
        "SELECT * FROM visible_topics",
        "SELECT * FROM visible_message_category_topics",
    ]);
    let categories = use_table_visible_message_categories();
    let subscriptions = use_table_visible_subscriptions();
    let topics = use_table_visible_topics();
    let category_topics = use_table_visible_message_category_topics();
    let add_subscription = use_reducer_add_subscription();
    let remove_subscription = use_reducer_remove_subscription();

    let account_id: u64 = user_info.mitgliedsnr.parse().unwrap_or(0);
    let email = user_info.email.clone().unwrap_or_default();

    let mut active_tab = use_signal(|| TopicTab::Sonstige);
    let mut user_picked_tab = use_signal(|| false);

    let active_cats: Vec<MessageCategory> = categories().into_iter().filter(|c| c.active).collect();

    let topic_ids: Vec<u64> = {
        let mut rows = topics();
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        rows.into_iter().map(|t| t.id).collect()
    };

    let links = category_topics();
    let categorized_ids: HashSet<u64> = links.iter().map(|l| l.category_id).collect();

    let sonstige_cats: Vec<MessageCategory> = active_cats
        .iter()
        .filter(|c| !categorized_ids.contains(&c.id))
        .cloned()
        .collect();

    let show_sonstige = !sonstige_cats.is_empty() || topic_ids.is_empty();

    // Keep the default tab in sync with loaded data until the user picks one.
    use_effect(move || {
        if user_picked_tab() {
            return;
        }
        let mut topics_sorted = topics();
        topics_sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        if let Some(first) = topics_sorted.first() {
            active_tab.set(TopicTab::Topic(first.id));
        } else {
            active_tab.set(TopicTab::Sonstige);
        }
    });

    let current = active_tab();
    let visible_cats: Vec<MessageCategory> = match current {
        TopicTab::Sonstige => sonstige_cats.clone(),
        TopicTab::Topic(topic_id) => {
            let cat_ids: HashSet<u64> = links
                .iter()
                .filter(|l| l.topic_id == topic_id)
                .map(|l| l.category_id)
                .collect();
            active_cats
                .into_iter()
                .filter(|c| cat_ids.contains(&c.id))
                .collect()
        }
    };

    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "envelope-check", class: "me-2" }
                        "{tid!(\"subscriptions-page-title\") }"
                    }
                    p { class: "text-muted mt-1",
                        "{tid!(\"subscriptions-page-description\") }"
                    }
                }
            }

            if !topic_ids.is_empty() || show_sonstige {
                Nav {
                    tabs: true,
                    class: "mb-3",
                    for topic_id in topic_ids.iter().copied() {
                        TopicTabButton {
                            key: "{topic_id}",
                            topic_id,
                            active: current == TopicTab::Topic(topic_id),
                            on_select: move |_| {
                                user_picked_tab.set(true);
                                active_tab.set(TopicTab::Topic(topic_id));
                            },
                        }
                    }
                    if show_sonstige {
                        NavItem {
                            NavLink {
                                active: current == TopicTab::Sonstige,
                                prevent_default: true,
                                onclick: move |_| {
                                    user_picked_tab.set(true);
                                    active_tab.set(TopicTab::Sonstige);
                                },
                                "{tid!(\"subscriptions-tab-other\") }"
                            }
                        }
                    }
                }
            }

            if visible_cats.is_empty() {
                Alert { color: Color::Info,
                    Icon { name: "info-circle", class: "me-2" }
                    if topic_ids.is_empty() && sonstige_cats.is_empty() {
                        "{tid!(\"subscriptions-empty\") }"
                    } else {
                        "{tid!(\"subscriptions-empty-category\") }"
                    }
                }
            } else {
                Row {
                    for cat in visible_cats {
                        {
                            let subscription = subscriptions().into_iter().find(|s| {
                                s.category_id == cat.id
                                    && s.subscriber_account_id == account_id
                                    && crate::pages::is_active_subscription(&s.status)
                            });
                            let sub_id = subscription.as_ref().map(|s| s.id);
                            let is_required = subscription
                                .as_ref()
                                .is_some_and(|s| matches!(s.status, SubscriptionStatus::RequiredSubscribed));
                            let cat_id = cat.id;
                            let email_for_sub = email.clone();
                            let add = add_subscription.clone();
                            let remove = remove_subscription.clone();
                            rsx! {
                                Col { md: ColumnSize::Span(6), lg: ColumnSize::Span(4), class: "mb-3",
                                    Card {
                                        class: if sub_id.is_some() { "h-100 border-dark bg-light" } else { "h-100 border-light" },
                                        body_class: "d-flex flex-column",
                                        header: rsx! {
                                            h5 { class: "card-title mb-0", "{cat.name}" }
                                            if sub_id.is_some() {
                                                Badge { color: Color::Success, class: "ms-2", "{tid!(\"subscriptions-subscribed\")}" }
                                            }

                                            if cat.visibility == CategoryVisibility::Public {
                                                Badge { color: Color::Info, class: "ms-2 align-middle", "{tid!(\"subscriptions-public\")}" }
                                            } else {
                                                Badge { color: Color::Warning, class: "ms-2 align-middle", "{tid!(\"subscriptions-private\")}" }
                                            }
                                        },
                                        body: rsx! {
                                            p { class: "card-text text-muted small flex-grow-1", "{cat.description}" }
                                            p { class: "card-text mb-3",
                                                small { class: "text-muted",
                                                    Icon { name: "envelope", class: "me-1" }
                                                    "{cat.email_address}"
                                                }
                                            }
                                            if is_required {
                                                Alert { color: Color::Info, class: "mt-auto mb-0 small",
                                                    Icon { name: "info-circle", class: "me-2" }
                                                    "{tid!(\"subscriptions-required\") }"
                                                }
                                            } else if let Some(id) = sub_id {
                                                Button {
                                                    color: Color::Danger,
                                                    size: Size::Sm,
                                                    class: "mt-auto",
                                                    onclick: move |_| {
                                                        info!("Unsubscribing from category {cat_id}");
                                                        if let Err(e) = remove(id) {
                                                            error!("remove_subscription failed: {e:?}");
                                                        }
                                                    },
                                                    Icon { name: "dash-circle", class: "me-1" }
                                                    "{tid!(\"subscriptions-unsubscribe\") }"
                                                }
                                            } else {
                                                Button {
                                                    color: Color::Success,
                                                    size: Size::Sm,
                                                    class: "mt-auto ",
                                                    onclick: move |_| {
                                                        info!("Subscribing to category {cat_id}");
                                                        if let Err(e) =
                                                            add(account_id, email_for_sub.clone(), cat_id)
                                                        {
                                                            error!("add_subscription failed: {e:?}");
                                                        }
                                                    },
                                                    Icon { name: "plus-circle", class: "me-1" }
                                                    "{tid!(\"subscriptions-subscribe\") }"
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

/// Tab button that always reads the topic name from the live `visible_topics` signal.
#[component]
fn TopicTabButton(topic_id: u64, active: bool, on_select: EventHandler<()>) -> Element {
    let topics = use_table_visible_topics();
    let name = use_memo(move || {
        topics()
            .into_iter()
            .find(|t| t.id == topic_id)
            .map(|t| t.name)
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
