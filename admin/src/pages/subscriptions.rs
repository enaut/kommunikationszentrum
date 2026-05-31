use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};

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
        div { class: "container-fluid mt-4",
            div { class: "row mb-3",
                div { class: "col",
                    h2 { class: "mb-0",
                        i { class: "bi bi-envelope-check me-2" }
                        "Meine Kategorien"
                    }
                    p { class: "text-muted mt-1",
                        "Wähle die Kategorien aus, über die du E-Mails empfangen möchtest."
                    }
                }
            }

            {
                let active_cats: Vec<_> = categories().into_iter().filter(|c| c.active).collect();
                if active_cats.is_empty() {
                    rsx! {
                        div { class: "alert alert-info",
                            i { class: "bi bi-info-circle me-2" }
                            "Keine Kategorien vorhanden."
                        }
                    }
                } else {
                    rsx! {
                        div { class: "row",
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
                                        div { class: "col-md-6 col-lg-4 mb-3",
                                            div {
                                                class: if sub_id.is_some() {
                                                    "card h-100 border-success"
                                                } else {
                                                    "card h-100"
                                                },
                                                div { class: "card-body d-flex flex-column",
                                                    div { class: "d-flex justify-content-between align-items-start mb-2",
                                                        h5 { class: "card-title mb-0", "{cat.name}" }
                                                        if sub_id.is_some() {
                                                            span { class: "badge bg-success ms-2", "Abonniert" }
                                                        }
                                                    }
                                                    p { class: "card-text text-muted small flex-grow-1",
                                                        "{cat.description}"
                                                    }
                                                    p { class: "card-text mb-3",
                                                        small { class: "text-muted",
                                                            i { class: "bi bi-envelope me-1" }
                                                            "{cat.email_address}"
                                                        }
                                                    }
                                                    if let Some(id) = sub_id {
                                                        button {
                                                            class: "btn btn-outline-danger btn-sm mt-auto",
                                                            onclick: move |_| {
                                                                info!("Unsubscribing from category {cat_id}");
                                                                if let Err(e) = remove(id) {
                                                                    error!("remove_subscription failed: {e:?}");
                                                                }
                                                            },
                                                            i { class: "bi bi-dash-circle me-1" }
                                                            "Abbestellen"
                                                        }
                                                    } else {
                                                        button {
                                                            class: "btn btn-success btn-sm mt-auto",
                                                            onclick: move |_| {
                                                                info!("Subscribing to category {cat_id}");
                                                                if let Err(e) =
                                                                    add(account_id, email_for_sub.clone(), cat_id)
                                                                {
                                                                    error!("add_subscription failed: {e:?}");
                                                                }
                                                            },
                                                            i { class: "bi bi-plus-circle me-1" }
                                                            "Abonnieren"
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
        }
    }
}
