use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::dioxus::use_reducer_admin_add_subscription;
use crate::module_bindings::{Account, SubscriptionStatus};
use crate::pages::category::subscribers::{parse_status, status_key, status_label, ALL_STATUSES};

/// Auto-select threshold: if the filtered list has fewer than this many entries,
/// the first result is selected automatically.
const AUTO_SELECT_THRESHOLD: usize = 20;

/// Target data for editing a subscriber's status in the edit modal.
#[derive(Clone, PartialEq, Debug)]
pub struct EditSubscriptionTarget {
    pub account_id: u64,
    pub name: String,
    pub email: String,
    pub status: SubscriptionStatus,
}

/// Modal for adding a new subscriber to the category.
#[component]
pub fn AddSubscriberModal(
    mut show: Signal<bool>,
    category_id: u64,
    available_accounts: Vec<Account>,
) -> Element {
    let admin_add_subscription = use_reducer_admin_add_subscription();

    let mut selected_account_id = use_signal(|| 0u64);
    let mut account_filter = use_signal(String::new);
    let mut selected_status = use_signal(|| SubscriptionStatus::ManuallySubscribed);
    let mut add_sub_error: Signal<Option<String>> = use_signal(|| None);

    // Reset state whenever the modal is shown
    use_effect(move || {
        if show() {
            selected_account_id.set(0);
            account_filter.set(String::new());
            selected_status.set(SubscriptionStatus::ManuallySubscribed);
            add_sub_error.set(None);
        }
    });

    let filter_lower = account_filter().to_lowercase();
    let filtered_accounts: Vec<_> = available_accounts
        .iter()
        .filter(|a| {
            filter_lower.is_empty()
                || a.name.to_lowercase().contains(&filter_lower)
                || a.email.to_lowercase().contains(&filter_lower)
        })
        .cloned()
        .collect();

    let available_for_filter = available_accounts.clone();
    let filter_count_text = tid!(
        "members-filter-count",
        filtered: filtered_accounts.len(),
        total: available_accounts.len()
    );

    rsx! {
        Modal {
            show,
            title: tid!("subscriber-add-title"),
            body: rsx! {
                if let Some(err) = add_sub_error.read().clone() {
                    Alert { color: Color::Danger, class: "mb-3", "{err}" }
                }
                if available_accounts.is_empty() {
                    p { class: "text-muted mb-0", "{tid!(\"subscriber-add-all-claimed\") }" }
                } else {
                    FormGroup { label: tid!("subscriber-member-label"),
                        InputGroup { class: "mb-2",
                            InputGroupText { Icon { name: "search" } }
                            Input {
                                r#type: "search",
                                placeholder: tid!("subscriber-search-placeholder"),
                                value: "{account_filter}",
                                oninput: move |e: FormEvent| {
                                    let new_val = e.value();
                                    let new_filter = new_val.to_lowercase();
                                    account_filter.set(new_val);

                                    let matched: Vec<_> = available_for_filter
                                        .iter()
                                        .filter(|a| {
                                            new_filter.is_empty()
                                                || a.name.to_lowercase().contains(&new_filter)
                                                || a.email.to_lowercase().contains(&new_filter)
                                        })
                                        .collect();

                                    if matched.len() < AUTO_SELECT_THRESHOLD {
                                        selected_account_id
                                            .set(matched.first().map(|a| a.id).unwrap_or(0));
                                    } else {
                                        selected_account_id.set(0);
                                    }
                                },
                            }
                        }
                        Select {
                            value: selected_account_id().to_string(),
                            onchange: move |e: FormEvent| {
                                if let Ok(id) = e.value().parse::<u64>() {
                                    selected_account_id.set(id);
                                }
                            },
                            option { value: "0",
                                if filtered_accounts.is_empty() {
                                    "{tid!(\"general-no-results\") }"
                                } else {
                                    "{tid!(\"subscriber-select-member\") }"
                                }
                            }
                            for acc in filtered_accounts.clone() {
                                option {
                                    key: "{acc.id}",
                                    value: "{acc.id}",
                                    "{acc.name} ({acc.email})"
                                }
                            }
                        }
                        if !filter_lower.is_empty() {
                            FormText {
                                "{filter_count_text}"
                            }
                        }
                    }

                    FormGroup { label: tid!("subscriber-status-label"),
                        Select {
                            value: status_key(&selected_status()).to_string(),
                            onchange: move |e: FormEvent| {
                                if let Some(s) = parse_status(&e.value()) {
                                    selected_status.set(s);
                                }
                            },
                            for s in ALL_STATUSES {
                                option {
                                    key: "{status_key(s)}",
                                    value: "{status_key(s)}",
                                    "{status_label(s)}"
                                }
                            }
                        }
                    }
                }
            },
            footer: rsx! {
                Button {
                    color: Color::Secondary,
                    onclick: move |_| show.set(false),
                    "{tid!(\"subscriber-cancel\") }"
                }
                Button {
                    color: Color::Primary,
                    disabled: selected_account_id() == 0,
                    onclick: move |_| {
                        let acc_id = selected_account_id();
                        if acc_id == 0 {
                            return;
                        }
                        let Some(acc) = available_accounts.iter().find(|a| a.id == acc_id) else {
                            add_sub_error.set(Some(tid!("subscriber-member-not-found")));
                            return;
                        };
                        let status = selected_status();
                        info!(
                            "Admin adding subscription: account={acc_id}, category={category_id}, status={status:?}"
                        );
                        match admin_add_subscription(acc_id, acc.email.clone(), category_id, status) {
                            Ok(()) => {
                                show.set(false);
                            }
                            Err(e) => {
                                error!("admin_add_subscription failed: {e:?}");
                                add_sub_error.set(Some(format!("{}: {e:?}", tid!("subscriber-error-prefix"))));
                            }
                        }
                    },
                    Icon { name: "check-lg", class: "me-2" }
                    "{tid!(\"subscriber-add\") }"
                }
            },
        }
    }
}

/// Modal for editing an existing subscriber's status.
#[component]
pub fn EditSubscriptionModal(
    mut show: Signal<bool>,
    category_id: u64,
    target: Signal<Option<EditSubscriptionTarget>>,
) -> Element {
    let admin_add_subscription = use_reducer_admin_add_subscription();

    let mut edit_status = use_signal(|| SubscriptionStatus::ManuallySubscribed);
    let mut edit_sub_error: Signal<Option<String>> = use_signal(|| None);

    // Sync state when target changes
    use_effect(move || {
        if let Some(t) = target() {
            edit_status.set(t.status);
            edit_sub_error.set(None);
        }
    });

    let target_val = target();
    let account_name = target_val
        .as_ref()
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let account_email = target_val
        .as_ref()
        .map(|t| t.email.clone())
        .unwrap_or_default();

    rsx! {
        Modal {
            show,
            title: tid!("subscriber-edit-title"),
            body: rsx! {
                if let Some(err) = edit_sub_error.read().clone() {
                    Alert { color: Color::Danger, class: "mb-3", "{err}" }
                }
                FormGroup { label: tid!("subscriber-member-label"),
                    Input {
                        r#type: "text",
                        value: "{account_name}",
                        disabled: true,
                        readonly: true,
                    }
                }
                FormGroup { label: tid!("subscriber-email-label"),
                    Input {
                        r#type: "text",
                        value: "{account_email}",
                        disabled: true,
                        readonly: true,
                    }
                }
                FormGroup { label: tid!("subscriber-status-label"),
                    Select {
                        value: status_key(&edit_status()).to_string(),
                        onchange: move |e: FormEvent| {
                            if let Some(s) = parse_status(&e.value()) {
                                edit_status.set(s);
                            }
                        },
                        for s in ALL_STATUSES {
                            option {
                                key: "{status_key(s)}",
                                value: "{status_key(s)}",
                                "{status_label(s)}"
                            }
                        }
                    }
                }
            },
            footer: rsx! {
                Button {
                    color: Color::Secondary,
                    onclick: move |_| show.set(false),
                    "{tid!(\"subscriber-cancel\") }"
                }
                Button {
                    color: Color::Primary,
                    onclick: move |_| {
                        let Some(t) = target() else { return };
                        let status = edit_status();
                        info!(
                            "Admin updating subscription: account={}, category={category_id}, status={status:?}",
                            t.account_id
                        );
                        match admin_add_subscription(t.account_id, t.email, category_id, status) {
                            Ok(()) => {
                                show.set(false);
                            }
                            Err(e) => {
                                error!("admin_add_subscription (edit) failed: {e:?}");
                                edit_sub_error.set(Some(format!("{}: {e:?}", tid!("subscriber-error-prefix"))));
                            }
                        }
                    },
                    Icon { name: "check-lg", class: "me-2" }
                    "{tid!(\"subscriber-save\") }"
                }
            },
        }
    }
}
