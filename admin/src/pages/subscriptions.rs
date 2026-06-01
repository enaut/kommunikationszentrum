use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{
    use_reducer_add_subscription, use_reducer_remove_subscription, use_table_message_categories,
    use_table_visible_subscriptions,
};
use crate::oauth::UserInfo;

/// Default view for all users: lists all active message categories and lets the
/// user subscribe or unsubscribe with a single button click.
#[component]
pub fn SubscriptionsPage(user_info: UserInfo) -> Element {
    let categories = use_table_message_categories();
    let subscriptions = use_table_visible_subscriptions();
    let add_subscription = use_reducer_add_subscription();
    let remove_subscription = use_reducer_remove_subscription();

    let account_id: u64 = user_info.mitgliedsnr.parse().unwrap_or(0);
    let email = user_info.email.clone().unwrap_or_default();

    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "envelope-check", class: "me-2" }
                        "Meine Themen"
                    }
                    p { class: "text-muted mt-1",
                        "Wähle die Themen aus, über die du E-Mails empfangen möchtest."
                    }
                }
            }

            {
                let active_cats: Vec<_> = categories()
                    .into_iter()
                    .filter(|c| c.active)
                    .collect();
                if active_cats.is_empty() {
                    rsx! {
                        Alert { color: Color::Info,
                            Icon { name: "info-circle", class: "me-2" }
                            "Keine Themen vorhanden."
                        }
                    }
                } else {
                    rsx! {
                        Row {
                            for cat in active_cats {
                                {
                                    let sub_id = subscriptions()
                                        .into_iter()
                                        .find(|s| {
                                            s.category_id == cat.id
                                                && s.subscriber_account_id == account_id
                                                && s.active
                                        })
                                        .map(|s| s.id);
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
                                                        Badge { color: Color::Success, class: "ms-2", "Abonniert" }
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
                                                    if let Some(id) = sub_id {
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
                                                            "Abbestellen"
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
                                                            "Abonnieren"
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
    }
}
