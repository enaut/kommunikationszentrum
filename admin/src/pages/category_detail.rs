use std::collections::HashSet;

use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{
    use_reducer_add_subscription, use_reducer_remove_subscription,
    use_reducer_update_message_category, use_table_message_categories, use_table_visible_accounts,
    use_table_visible_subscriptions,
};

/// Admin-only detail/edit view for a single message category (mailing list
/// topic). Allows editing name and description, shows the (immutable) email
/// address, and manages the list of subscribers.
#[component]
pub fn CategoryDetailPage(category_id: u64, on_back: EventHandler<()>) -> Element {
    let categories = use_table_message_categories();
    let subscriptions = use_table_visible_subscriptions();
    let accounts = use_table_visible_accounts();

    let update_category = use_reducer_update_message_category();
    let add_subscription = use_reducer_add_subscription();
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

    // "Add subscriber" modal state.
    let mut show_add_modal = use_signal(|| false);
    let mut selected_account_id: Signal<u64> = use_signal(|| 0);
    let mut add_sub_error: Signal<Option<String>> = use_signal(|| None);

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
        .filter(|s| s.category_id == category_id && crate::pages::is_active_subscription(&s.status))
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
                                        selected_account_id.set(0);
                                        add_sub_error.set(None);
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
                                                th { class: "text-end", "Aktionen" }
                                            }
                                        }
                                        tbody {
                                            for (sub , account) in subscriber_rows.clone() {
                                                {
                                                    let sub_id = sub.id;
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
                                                    rsx! {
                                                        tr {
                                                            td { "{name_disp}" }
                                                            td {
                                                                code { "{email_disp}" }
                                                            }
                                                            td { class: "text-end",
                                                                Button {
                                                                    color: Color::Danger,
                                                                    size: Size::Sm,
                                                                    onclick: move |_| {
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

            Modal {
                show: show_add_modal,
                title: "Abonnent hinzufügen",
                body: rsx! {
                    if let Some(err) = add_sub_error.read().clone() {
                        Alert { color: Color::Danger, class: "mb-3", "{err}" }
                    }
                    if available_accounts.is_empty() {
                        p { class: "text-muted mb-0", "Alle Mitglieder sind bereits abonniert." }
                    } else {
                        label { class: "form-label", "Mitglied" }
                        select {
                            class: "form-select",
                            onchange: move |e| {
                                if let Ok(id) = e.value().parse::<u64>() {
                                    selected_account_id.set(id);
                                }
                            },
                            option { value: "0", "– Mitglied wählen –" }
                            for acc in available_accounts.clone() {
                                {
                                    let val = acc.id.to_string();
                                    let option_label = format!("{} ({})", acc.name, acc.email);
                                    rsx! {
                                        option { value: "{val}", "{option_label}" }
                                    }
                                }
                            }
                        }
                    }
                },
                footer: rsx! {
                    Button {
                        color: Color::Secondary,
                        onclick: move |_| show_add_modal.set(false),
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
                            let Some(acc) = accounts().into_iter().find(|a| a.id == acc_id) else {
                                add_sub_error.set(Some("Mitglied nicht gefunden.".to_string()));
                                return;
                            };
                            info!("Adding subscription: account={acc_id}, category={category_id}");
                            match add_subscription(acc_id, acc.email.clone(), category_id) {
                                Ok(()) => {
                                    show_add_modal.set(false);
                                }
                                Err(e) => {
                                    error!("add_subscription failed: {e:?}");
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
}
