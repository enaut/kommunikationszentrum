use std::collections::HashSet;

use ::dioxus::{logger::tracing::error, prelude::*};
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{
    use_reducer_update_message_category, use_subscription, use_table_visible_accounts,
    use_table_visible_message_categories, use_table_visible_message_category_topics,
    use_table_visible_subscriptions, use_table_visible_topics,
};
use crate::module_bindings::{CategoryVisibility, MessageCategory};
use crate::pages::category::modals::{
    AddSubscriberModal, EditSubscriptionModal, EditSubscriptionTarget,
};
use crate::pages::category::subscribers::CategorySubscribersCard;
use crate::pages::category::topics::CategoryTopicsCard;

/// Form card for editing a category's name, description, and visibility.
#[component]
pub fn CategoryDetailsCard(
    category: MessageCategory,
    mut name: Signal<String>,
    mut description: Signal<String>,
    mut visibility: Signal<CategoryVisibility>,
    mut save_message: Signal<Option<(String, Color)>>,
) -> Element {
    let update_category = use_reducer_update_message_category();
    let category_id = category.id;
    let visibility_value = match visibility() {
        CategoryVisibility::Public => "Public",
        CategoryVisibility::Private => "Private",
    };

    rsx! {
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
                FormGroup { label: "Name",
                    Input {
                        r#type: "text",
                        value: "{name}",
                        oninput: move |e: FormEvent| name.set(e.value()),
                    }
                }
                FormGroup { label: "Beschreibung",
                    Textarea {
                        rows: 3,
                        value: "{description}",
                        oninput: move |e: FormEvent| description.set(e.value()),
                    }
                }
                FormGroup { label: "Sichtbarkeit",
                    Select {
                        value: visibility_value,
                        onchange: move |e: FormEvent| {
                            match e.value().as_str() {
                                "Public" => visibility.set(CategoryVisibility::Public),
                                "Private" => visibility.set(CategoryVisibility::Private),
                                _ => {}
                            }
                        },
                        option { value: "Public", "Öffentlich" }
                        option { value: "Private", "Privat" }
                    }
                    FormText {
                        "Öffentliche Themen sind für alle Mitglieder sichtbar. Private Themen sind nur für Administratoren und abonnierte Mitglieder sichtbar."
                    }
                }
                FormGroup { label: "E-Mail-Adresse",
                    Input {
                        r#type: "text",
                        value: "{category.email_address}",
                        disabled: true,
                        readonly: true,
                    }
                    FormText { "Die E-Mail-Adresse ist fest mit dem Thema verknüpft und kann nicht geändert werden." }
                }
                Button {
                    color: Color::Primary,
                    disabled: name.read().trim().is_empty(),
                    onclick: move |_| {
                        let n = name.read().clone();
                        let d = description.read().clone();
                        let v = visibility.read().clone();
                        match update_category(category_id, n, d, Some(v)) {
                            Ok(()) => {
                                save_message
                                    .set(Some(("Änderungen gespeichert.".to_string(), Color::Success)));
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

/// Admin-only detail/edit view for a single message category (mailing list topic).
#[component]
pub fn CategoryDetailPage(category_id: u64, on_back: EventHandler<()>) -> Element {
    use_subscription(&[
        "SELECT * FROM visible_accounts",
        "SELECT * FROM visible_message_categories",
        "SELECT * FROM visible_subscriptions",
        "SELECT * FROM visible_topics",
        "SELECT * FROM visible_message_category_topics",
    ]);
    let categories = use_table_visible_message_categories();
    let subscriptions = use_table_visible_subscriptions();
    let accounts = use_table_visible_accounts();
    let topics = use_table_visible_topics();
    let category_topics = use_table_visible_message_category_topics();

    let category = use_memo(move || categories().into_iter().find(|c| c.id == category_id));

    // Local edit state, seeded once from the loaded category.
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut visibility = use_signal(|| CategoryVisibility::Public);
    let mut initialized = use_signal(|| false);
    let save_message: Signal<Option<(String, Color)>> = use_signal(|| None);
    let topics_message: Signal<Option<(String, Color)>> = use_signal(|| None);
    let new_topic_name = use_signal(String::new);
    let renaming_topic_id: Signal<Option<u64>> = use_signal(|| None);
    let rename_draft = use_signal(String::new);

    use_effect(move || {
        if let Some(cat) = category() {
            if !initialized() {
                name.set(cat.name.clone());
                description.set(cat.description.clone());
                visibility.set(cat.visibility);
                initialized.set(true);
            }
        }
    });

    // Modal display states
    let show_add_modal = use_signal(|| false);
    let show_edit_modal = use_signal(|| false);
    let edit_target: Signal<Option<EditSubscriptionTarget>> = use_signal(|| None);

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

    let subscribed_account_ids: HashSet<u64> = category_subscriptions
        .iter()
        .map(|s| s.subscriber_account_id)
        .collect();

    let available_accounts: Vec<_> = accounts()
        .into_iter()
        .filter(|a| !subscribed_account_ids.contains(&a.id))
        .collect();

    let assigned_topic_ids: HashSet<u64> = category_topics()
        .into_iter()
        .filter(|link| link.category_id == category_id)
        .map(|link| link.topic_id)
        .collect();

    // Stable ids only — names come from the live `topics` signal inside each row.
    let topic_ids: Vec<u64> = {
        let mut rows = topics();
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        rows.into_iter().map(|t| t.id).collect()
    };

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
                            Badge {
                                color: Color::Success,
                                class: "ms-2 align-middle",
                                "Aktiv"
                            }
                        } else {
                            Badge {
                                color: Color::Secondary,
                                class: "ms-2 align-middle",
                                "Inaktiv"
                            }
                        }
                        if cat.visibility == CategoryVisibility::Public {
                            Badge {
                                color: Color::Info,
                                class: "ms-2 align-middle",
                                "Öffentlich"
                            }
                        } else {
                            Badge {
                                color: Color::Warning,
                                class: "ms-2 align-middle",
                                "Privat"
                            }
                        }
                    }
                }
            }

            Row { class: "mb-4",
                Col { lg: ColumnSize::Span(6), class: "mb-3",
                    CategoryDetailsCard {
                        category: cat,
                        name,
                        description,
                        visibility,
                        save_message,
                    }
                }
                Col { lg: ColumnSize::Span(6), class: "mb-3",
                    CategoryTopicsCard {
                        category_id,
                        assigned_topic_ids,
                        topic_ids,
                        topics_message,
                        new_topic_name,
                        renaming_topic_id,
                        rename_draft,
                    }
                }
            }

            Row {
                Col { xs: ColumnSize::Span(12),
                    CategorySubscribersCard {
                        category_id,
                        show_add_modal,
                        show_edit_modal,
                        edit_target,
                    }
                }
            }

            AddSubscriberModal {
                show: show_add_modal,
                category_id,
                available_accounts,
            }

            EditSubscriptionModal { show: show_edit_modal, category_id, target: edit_target }
        }
    }
}
