use std::collections::HashSet;

use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{
    use_reducer_admin_add_subscription, use_reducer_remove_subscription,
    use_reducer_update_message_category, use_table_message_categories, use_table_visible_accounts,
    use_table_visible_subscriptions,
};
use crate::module_bindings::{Account, SubscriptionStatus};

/// German display label for each subscription status.
fn status_label(status: &SubscriptionStatus) -> &'static str {
    match status {
        SubscriptionStatus::ManuallySubscribed => "Manuell abonniert",
        SubscriptionStatus::AutomaticallySubscribed => "Automatisch abonniert",
        SubscriptionStatus::ManuallyUnsubscribed => "Manuell abgemeldet",
        SubscriptionStatus::AutomaticallyUnsubscribed => "Automatisch abgemeldet",
        SubscriptionStatus::LinkUnsubscribed => "Per Link abgemeldet",
    }
}

/// Bootstrap color for each subscription status badge.
fn status_color(status: &SubscriptionStatus) -> Color {
    match status {
        SubscriptionStatus::ManuallySubscribed => Color::Success,
        SubscriptionStatus::AutomaticallySubscribed => Color::Info,
        SubscriptionStatus::ManuallyUnsubscribed => Color::Warning,
        SubscriptionStatus::AutomaticallyUnsubscribed => Color::Secondary,
        SubscriptionStatus::LinkUnsubscribed => Color::Danger,
    }
}

/// Parse a select option value string back to a SubscriptionStatus.
fn parse_status(s: &str) -> Option<SubscriptionStatus> {
    match s {
        "ManuallySubscribed" => Some(SubscriptionStatus::ManuallySubscribed),
        "AutomaticallySubscribed" => Some(SubscriptionStatus::AutomaticallySubscribed),
        "ManuallyUnsubscribed" => Some(SubscriptionStatus::ManuallyUnsubscribed),
        "AutomaticallyUnsubscribed" => Some(SubscriptionStatus::AutomaticallyUnsubscribed),
        "LinkUnsubscribed" => Some(SubscriptionStatus::LinkUnsubscribed),
        _ => None,
    }
}

/// Stable string key for a SubscriptionStatus (used as <option value>).
fn status_key(status: &SubscriptionStatus) -> &'static str {
    match status {
        SubscriptionStatus::ManuallySubscribed => "ManuallySubscribed",
        SubscriptionStatus::AutomaticallySubscribed => "AutomaticallySubscribed",
        SubscriptionStatus::ManuallyUnsubscribed => "ManuallyUnsubscribed",
        SubscriptionStatus::AutomaticallyUnsubscribed => "AutomaticallyUnsubscribed",
        SubscriptionStatus::LinkUnsubscribed => "LinkUnsubscribed",
    }
}

/// All status variants in display order.
const ALL_STATUSES: &[SubscriptionStatus] = &[
    SubscriptionStatus::ManuallySubscribed,
    SubscriptionStatus::AutomaticallySubscribed,
    SubscriptionStatus::ManuallyUnsubscribed,
    SubscriptionStatus::AutomaticallyUnsubscribed,
    SubscriptionStatus::LinkUnsubscribed,
];

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

/// Admin-only detail/edit view for a single message category (mailing list topic).
#[component]
pub fn CategoryDetailPage(category_id: u64, on_back: EventHandler<()>) -> Element {
    let categories = use_table_message_categories();
    let subscriptions = use_table_visible_subscriptions();
    let accounts = use_table_visible_accounts();

    let update_category = use_reducer_update_message_category();
    let remove_subscription = use_reducer_remove_subscription();

    let category = use_memo(move || categories().into_iter().find(|c| c.id == category_id));

    // Local edit state, seeded once from the loaded category.
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut initialized = use_signal(|| false);
    let mut save_message: Signal<Option<(String, Color)>> = use_signal(|| None);

    use_effect(move || {
        if let Some(cat) = category() {
            if !initialized() {
                name.set(cat.name.clone());
                description.set(cat.description.clone());
                initialized.set(true);
            }
        }
    });

    // Modal display states
    let mut show_add_modal = use_signal(|| false);
    let mut show_edit_modal = use_signal(|| false);
    let mut edit_target: Signal<Option<EditSubscriptionTarget>> = use_signal(|| None);

    let Some(cat) = category() else {
        return rsx! {
            Container { fluid: true, class: "mt-4",
                Alert { color: Color::Warning, class: "d-flex align-items-center",
                    Icon { name: "exclamation-triangle", class: "me-2" }
                    "Thema nicht gefunden (evtl. gelöscht)."
                }
                Button {
                    color: Color::Secondary,
                    onclick: move |_| on_back.call(()),
                    Icon { name: "arrow-left", class: "me-2" }
                    "Zurück zur Übersicht"
                }
            }
        };
    };

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

    let subscribed_account_ids: HashSet<u64> = category_subscriptions
        .iter()
        .map(|s| s.subscriber_account_id)
        .collect();

    let available_accounts: Vec<_> = all_accounts
        .into_iter()
        .filter(|a| !subscribed_account_ids.contains(&a.id))
        .collect();

    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3",
                Col {
                    Button {
                        color: Color::Secondary,
                        size: Size::Sm,
                        class: "mb-2",
                        onclick: move |_| on_back.call(()),
                        Icon { name: "arrow-left", class: "me-2" }
                        "Zurück zur Übersicht"
                    }
                    h2 { class: "mb-0",
                        Icon { name: "tag-fill", class: "me-2" }
                        "{cat.name}"
                        if cat.active {
                            Badge { color: Color::Success, class: "ms-2 align-middle", "Aktiv" }
                        } else {
                            Badge { color: Color::Secondary, class: "ms-2 align-middle", "Inaktiv" }
                        }
                    }
                }
            }

            Row { class: "mb-4",
                Col { lg: ColumnSize::Span(6), class: "mb-3",
                    Card {
                        class: "shadow-sm h-100",
                        header_class: "bg-primary text-white",
                        header: rsx! {
                            h5 { class: "card-title mb-0",
                                Icon { name: "pencil-square", class: "me-2" }
                                "Details bearbeiten"
                            }
                        },
                        body: rsx! {
                            if let Some((msg, color)) = save_message.read().clone() {
                                Alert { color, class: "mb-3", "{msg}" }
                            }
                            div { class: "mb-3",
                                label { class: "form-label", "Name" }
                                input {
                                    class: "form-control",
                                    r#type: "text",
                                    value: "{name}",
                                    oninput: move |e| name.set(e.value()),
                                }
                            }
                            div { class: "mb-3",
                                label { class: "form-label", "Beschreibung" }
                                textarea {
                                    class: "form-control",
                                    rows: "3",
                                    value: "{description}",
                                    oninput: move |e| description.set(e.value()),
                                }
                            }
                            div { class: "mb-3",
                                label { class: "form-label", "E-Mail-Adresse" }
                                input {
                                    class: "form-control",
                                    r#type: "text",
                                    value: "{cat.email_address}",
                                    disabled: true,
                                    readonly: true,
                                }
                                div { class: "form-text",
                                    "Die E-Mail-Adresse ist fest mit dem Thema verknüpft und kann nicht geändert werden."
                                }
                            }
                            Button {
                                color: Color::Primary,
                                disabled: name.read().trim().is_empty(),
                                onclick: move |_| {
                                    let n = name.read().clone();
                                    let d = description.read().clone();
                                    match update_category(category_id, n, d) {
                                        Ok(()) => {
                                            save_message
                                                .set(
                                                    Some(("Änderungen gespeichert.".to_string(), Color::Success)),
                                                );
                                        }
                                        Err(e) => {
                                            error!("update_message_category failed: {e:?}");
                                            save_message
                                                .set(Some((format!("Fehler beim Speichern: {e:?}"), Color::Danger)));
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

            Row {
                Col { xs: ColumnSize::Span(12),
                    Card {
                        class: "shadow-sm",
                        header_class: "bg-primary text-white",
                        body_class: "p-0",
                        header: rsx! {
                            div { class: "d-flex justify-content-between align-items-center",
                                h5 { class: "card-title mb-0",
                                    Icon { name: "people-fill", class: "me-2" }
                                    "Abonnenten"
                                    span { class: "badge bg-white text-primary ms-2", "{subscriber_rows.len()}" }
                                }
                                Button {
                                    color: Color::Light,
                                    size: Size::Sm,
                                    onclick: move |_| {
                                        show_add_modal.set(true);
                                    },
                                    Icon { name: "plus-lg", class: "me-1" }
                                    "Hinzufügen"
                                }
                            }
                        },
                        body: rsx! {
                            if subscriber_rows.is_empty() {
                                div { class: "p-4 text-muted",
                                    Icon { name: "inbox", class: "me-2" }
                                    "Keine Abonnenten."
                                }
                            } else {
                                div { class: "table-responsive",
                                    table { class: "table table-hover mb-0",
                                        thead { class: "table-light",
                                            tr {
                                                th { "Name" }
                                                th { "E-Mail" }
                                                th { "Status" }
                                                th { class: "text-end", "Aktionen" }
                                            }
                                        }
                                        tbody {
                                            for (sub , account) in subscriber_rows.clone() {
                                                {
                                                    let sub_id = sub.id;
                                                    let sub_account_id = sub.subscriber_account_id;
                                                    let sub_status = sub.status;
                                                    let remove = remove_subscription.clone();
                                                    let (name_disp, email_disp) = match &account {
                                                        Some(a) => (a.name.clone(), a.email.clone()),
                                                        None => {
                                                            (
                                                                format!("Mitglied #{}", sub.subscriber_account_id),
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
                                                                Badge {
                                                                    color: badge_color,
                                                                    "{badge_label}"
                                                                }
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
                                                                    "Entfernen"
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
                        },
                    }
                }
            }

            AddSubscriberModal {
                show: show_add_modal,
                category_id,
                available_accounts,
            }

            EditSubscriptionModal {
                show: show_edit_modal,
                category_id,
                target: edit_target,
            }
        }
    }
}

/// Modal for adding a new subscriber to the category.
#[component]
fn AddSubscriberModal(
    show: Signal<bool>,
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
fn EditSubscriptionModal(
    show: Signal<bool>,
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
