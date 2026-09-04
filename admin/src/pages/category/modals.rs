use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

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

    rsx! {
        Modal {
            show,
            title: "Abonnent hinzufügen",
            body: rsx! {
                if let Some(err) = add_sub_error.read().clone() {
                    Alert { color: Color::Danger, class: "mb-3", "{err}" }
                }
                if available_accounts.is_empty() {
                    p { class: "text-muted mb-0", "Alle Mitglieder sind bereits abonniert." }
                } else {
                    div { class: "mb-3",
                        label { class: "form-label", "Mitglied" }
                        div { class: "input-group mb-2",
                            span { class: "input-group-text",
                                Icon { name: "search" }
                            }
                            input {
                                class: "form-control",
                                r#type: "search",
                                placeholder: "Name oder E-Mail filtern …",
                                value: "{account_filter}",
                                oninput: move |e| {
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
                        select {
                            class: "form-select",
                            onchange: move |e| {
                                if let Ok(id) = e.value().parse::<u64>() {
                                    selected_account_id.set(id);
                                }
                            },
                            option { value: "0",
                                selected: selected_account_id() == 0,
                                if filtered_accounts.is_empty() {
                                    "– Keine Ergebnisse –"
                                } else {
                                    "– Mitglied wählen –"
                                }
                            }
                            for acc in filtered_accounts.clone() {
                                {
                                    let val = acc.id.to_string();
                                    let option_label = format!("{} ({})", acc.name, acc.email);
                                    let is_selected = acc.id == selected_account_id();
                                    rsx! {
                                        option {
                                            key: "{acc.id}",
                                            value: "{val}",
                                            selected: is_selected,
                                            "{option_label}"
                                        }
                                    }
                                }
                            }
                        }
                        if !filter_lower.is_empty() {
                            div { class: "form-text",
                                "{filtered_accounts.len()} von {available_accounts.len()} Mitgliedern"
                            }
                        }
                    }

                    div { class: "mb-3",
                        label { class: "form-label", "Status" }
                        select {
                            class: "form-select",
                            onchange: move |e| {
                                if let Some(s) = parse_status(&e.value()) {
                                    selected_status.set(s);
                                }
                            },
                            for s in ALL_STATUSES {
                                {
                                    let key = status_key(s);
                                    let label = status_label(s);
                                    let is_selected = *s == selected_status();
                                    rsx! {
                                        option {
                                            key: "{key}",
                                            value: "{key}",
                                            selected: is_selected,
                                            "{label}"
                                        }
                                    }
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
                    "Abbrechen"
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
                            add_sub_error.set(Some("Mitglied nicht gefunden.".to_string()));
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
                                add_sub_error.set(Some(format!("Fehler: {e:?}")));
                            }
                        }
                    },
                    Icon { name: "check-lg", class: "me-2" }
                    "Hinzufügen"
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
            title: "Abonnement bearbeiten",
            body: rsx! {
                if let Some(err) = edit_sub_error.read().clone() {
                    Alert { color: Color::Danger, class: "mb-3", "{err}" }
                }
                div { class: "mb-3",
                    label { class: "form-label", "Mitglied" }
                    input {
                        class: "form-control",
                        r#type: "text",
                        value: "{account_name}",
                        disabled: true,
                        readonly: true,
                    }
                }
                div { class: "mb-3",
                    label { class: "form-label", "E-Mail" }
                    input {
                        class: "form-control",
                        r#type: "text",
                        value: "{account_email}",
                        disabled: true,
                        readonly: true,
                    }
                }
                div { class: "mb-3",
                    label { class: "form-label", "Status" }
                    select {
                        class: "form-select",
                        onchange: move |e| {
                            if let Some(s) = parse_status(&e.value()) {
                                edit_status.set(s);
                            }
                        },
                        for s in ALL_STATUSES {
                            {
                                let key = status_key(s);
                                let label = status_label(s);
                                let is_selected = *s == edit_status();
                                rsx! {
                                    option {
                                        key: "{key}",
                                        value: "{key}",
                                        selected: is_selected,
                                        "{label}"
                                    }
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
                    "Abbrechen"
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
                                edit_sub_error.set(Some(format!("Fehler: {e:?}")));
                            }
                        }
                    },
                    Icon { name: "check-lg", class: "me-2" }
                    "Speichern"
                }
            },
        }
    }
}
