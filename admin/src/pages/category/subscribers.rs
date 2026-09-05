use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::dioxus::{
    use_reducer_remove_subscription, use_table_visible_accounts, use_table_visible_subscriptions,
};
use crate::module_bindings::SubscriptionStatus;
use crate::pages::category::modals::EditSubscriptionTarget;

/// Localized display label for each subscription status.
pub fn status_label(status: &SubscriptionStatus) -> String {
    match status {
        SubscriptionStatus::ManuallySubscribed => tid!("subscription-status-manually-subscribed"),
        SubscriptionStatus::AutomaticallySubscribed => {
            tid!("subscription-status-automatically-subscribed")
        }
        SubscriptionStatus::ManuallyUnsubscribed => {
            tid!("subscription-status-manually-unsubscribed")
        }
        SubscriptionStatus::AutomaticallyUnsubscribed => {
            tid!("subscription-status-automatically-unsubscribed")
        }
        SubscriptionStatus::LinkUnsubscribed => tid!("subscription-status-link-unsubscribed"),
        SubscriptionStatus::RequiredSubscribed => tid!("subscription-status-required-subscribed"),
    }
}

/// Bootstrap color for each subscription status badge.
pub fn status_color(status: &SubscriptionStatus) -> Color {
    match status {
        SubscriptionStatus::ManuallySubscribed => Color::Success,
        SubscriptionStatus::RequiredSubscribed => Color::Primary,
        SubscriptionStatus::AutomaticallySubscribed => Color::Info,
        SubscriptionStatus::ManuallyUnsubscribed => Color::Warning,
        SubscriptionStatus::AutomaticallyUnsubscribed => Color::Secondary,
        SubscriptionStatus::LinkUnsubscribed => Color::Danger,
    }
}

/// Parse a select option value string back to a SubscriptionStatus.
pub fn parse_status(s: &str) -> Option<SubscriptionStatus> {
    match s {
        "ManuallySubscribed" => Some(SubscriptionStatus::ManuallySubscribed),
        "AutomaticallySubscribed" => Some(SubscriptionStatus::AutomaticallySubscribed),
        "ManuallyUnsubscribed" => Some(SubscriptionStatus::ManuallyUnsubscribed),
        "AutomaticallyUnsubscribed" => Some(SubscriptionStatus::AutomaticallyUnsubscribed),
        "LinkUnsubscribed" => Some(SubscriptionStatus::LinkUnsubscribed),
        "RequiredSubscribed" => Some(SubscriptionStatus::RequiredSubscribed),
        _ => None,
    }
}

/// Stable string key for a SubscriptionStatus (used as <option value>).
pub fn status_key(status: &SubscriptionStatus) -> &'static str {
    match status {
        SubscriptionStatus::ManuallySubscribed => "ManuallySubscribed",
        SubscriptionStatus::AutomaticallySubscribed => "AutomaticallySubscribed",
        SubscriptionStatus::ManuallyUnsubscribed => "ManuallyUnsubscribed",
        SubscriptionStatus::AutomaticallyUnsubscribed => "AutomaticallyUnsubscribed",
        SubscriptionStatus::LinkUnsubscribed => "LinkUnsubscribed",
        SubscriptionStatus::RequiredSubscribed => "RequiredSubscribed",
    }
}

/// All status variants in display order.
pub const ALL_STATUSES: &[SubscriptionStatus] = &[
    SubscriptionStatus::ManuallySubscribed,
    SubscriptionStatus::AutomaticallySubscribed,
    SubscriptionStatus::ManuallyUnsubscribed,
    SubscriptionStatus::AutomaticallyUnsubscribed,
    SubscriptionStatus::LinkUnsubscribed,
    SubscriptionStatus::RequiredSubscribed,
];

/// Card displaying the list of subscribers for a category with removal and edit actions.
#[component]
pub fn CategorySubscribersCard(
    category_id: u64,
    mut show_add_modal: Signal<bool>,
    mut show_edit_modal: Signal<bool>,
    mut edit_target: Signal<Option<EditSubscriptionTarget>>,
) -> Element {
    let subscriptions = use_table_visible_subscriptions();
    let accounts = use_table_visible_accounts();
    let remove_subscription = use_reducer_remove_subscription();

    let category_subscriptions: Vec<_> = subscriptions()
        .into_iter()
        .filter(|s| s.category_id == category_id)
        .collect();

    let all_accounts = accounts();
    let subscriber_rows: Vec<_> = category_subscriptions
        .iter()
        .map(|sub| {
            let account = all_accounts
                .iter()
                .find(|a| a.id == sub.subscriber_account_id)
                .cloned();
            (sub.clone(), account)
        })
        .collect();

    let active_subscriber_count = category_subscriptions
        .iter()
        .filter(|sub| crate::pages::is_active_subscription(&sub.status))
        .count();

    rsx! {
        Card {
            class: "shadow-sm",
            header_class: "bg-primary text-white",
            body_class: "p-0",
            header: rsx! {
                div { class: "d-flex justify-content-between align-items-center",
                    h5 { class: "card-title mb-0",
                        Icon { name: "people-fill", class: "me-2" }
                        "{tid!(\"category-table-th-subscribers\") }"
                        span { class: "badge bg-white text-primary ms-2", "{active_subscriber_count}" }
                    }
                    Button {
                        color: Color::Light,
                        size: Size::Sm,
                        onclick: move |_| {
                            show_add_modal.set(true);
                        },
                        Icon { name: "plus-lg", class: "me-1" }
                        "{tid!(\"subscriber-add\") }"
                    }
                }
            },
            body: rsx! {
                if subscriber_rows.is_empty() {
                    div { class: "p-4 text-muted",
                        Icon { name: "inbox", class: "me-2" }
                        "{tid!(\"category-table-empty\") }"
                    }
                } else {
                    Table { hover: true, responsive: true, class: "mb-0",
                        thead { class: "table-light",
                            tr {
                                th { "{tid!(\"members-table-name\")}" }
                                th { "{tid!(\"members-table-email\")}" }
                                th { "{tid!(\"members-table-status\")}" }
                                th { class: "text-end", "{tid!(\"members-table-action\")}" }
                            }
                        }
                        tbody {
                            for (sub, account) in subscriber_rows {
                                {
                                    let sub_id = sub.id;
                                    let sub_account_id = sub.subscriber_account_id;
                                    let sub_status = sub.status;
                                    let remove = remove_subscription.clone();
                                    let (name_disp, email_disp) = match &account {
                                        Some(a) => (a.name.clone(), a.email.clone()),
                                        None => {
                                            (
                                                format!("{} #{}", tid!("subscriber-member-label"), sub.subscriber_account_id),
                                                sub.subscriber_email.clone(),
                                            )
                                        }
                                    };
                                    let badge_color = status_color(&sub.status);
                                    let badge_label = status_label(&sub.status);
                                    let row_target = EditSubscriptionTarget {
                                        account_id: sub_account_id,
                                        name: name_disp.clone(),
                                        email: email_disp.clone(),
                                        status: sub_status,
                                    };
                                    rsx! {
                                        tr {
                                            key: "{sub_id}",
                                            style: "cursor: pointer;",
                                            onclick: move |_| {
                                                edit_target.set(Some(row_target.clone()));
                                                show_edit_modal.set(true);
                                            },
                                            td { "{name_disp}" }
                                            td {
                                                code { "{email_disp}" }
                                            }
                                            td {
                                                Badge { color: badge_color, "{badge_label}" }
                                            }
                                            td { class: "text-end",
                                                Button {
                                                    color: Color::Danger,
                                                    size: Size::Sm,
                                                    onclick: move |evt: MouseEvent| {
                                                        evt.stop_propagation();
                                                        info!("Removing subscription {sub_id}");
                                                        if let Err(e) = remove(sub_id) {
                                                            error!("remove_subscription failed: {e:?}");
                                                        }
                                                    },
                                                    Icon { name: "trash", class: "me-1" }
                                                    "{tid!(\"subscriber-remove\") }"
                                                }
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
