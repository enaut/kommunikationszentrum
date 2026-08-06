use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{
    use_procedure_provision_message_category, use_reducer_remove_message_category,
    use_subscription, use_table_visible_message_categories, use_table_visible_subscriptions,
};
use crate::module_bindings::CategoryVisibility;
use crate::pages::category_detail::CategoryDetailPage;

/// Admin-only view: lists all message categories with inline add and delete controls.
#[component]
pub fn CategoriesPage() -> Element {
    use_subscription(&[
        "SELECT * FROM visible_message_categories",
        "SELECT * FROM visible_subscriptions",
    ]);
    let categories = use_table_visible_message_categories();
    let subscriptions = use_table_visible_subscriptions();
    // New generated hook returns (invoke, result_signal).
    let (add_invoke, add_result) = use_procedure_provision_message_category();
    let remove_category = use_reducer_remove_message_category();

    // When set, the detail/edit page for this category is shown instead of the list.
    let mut selected_category: Signal<Option<u64>> = use_signal(|| None);

    let mut name = use_signal(String::new);
    let mut email_address = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut visibility = use_signal(|| CategoryVisibility::Public);
    let add_error: Signal<Option<(String, Color)>> = use_signal(|| None);
    let is_sending = use_signal(|| false);

    // React to procedure result signal and update UI accordingly.
    {
        let mut add_result = add_result.clone();
        let mut name = name.clone();
        let mut email_address = email_address.clone();
        let mut description = description.clone();
        let mut add_error = add_error.clone();
        let mut is_sending = is_sending.clone();

        use_effect(move || {
            if let Some(res) = add_result() {
                // request finished
                is_sending.set(false);
                match res {
                    Ok(inner) => match inner {
                        Ok(()) => {
                            name.set(String::new());
                            email_address.set(String::new());
                            description.set(String::new());
                            visibility.set(CategoryVisibility::Public);
                            add_error.set(Some((
                                "Neues Thema erfolgreich erstellt!".to_string(),
                                Color::Success,
                            )));
                        }
                        Err(proc_err) => {
                            error!("provision_message_category failed: {proc_err}");
                            add_error.set(Some((proc_err, Color::Danger)));
                        }
                    },
                    Err(internal_err) => {
                        error!("provision_message_category internal error: {internal_err}");
                        add_error.set(Some((internal_err, Color::Danger)));
                    }
                }

                // clear the result so the next invocation can be observed
                add_result.set(None);
            }
        });
    }

    rsx! {
        if let Some(id) = selected_category() {
            CategoryDetailPage { category_id: id, on_back: move |_| selected_category.set(None) }
        } else {
            Container { fluid: true, class: "mt-4",
                Row { class: "mb-3",
                    Col {
                        h2 { class: "mb-0",
                            Icon { name: "tags-fill", class: "me-2" }
                            "Themen"
                        }
                    }
                }

            // Add form
            Row { class: "mb-4",
                Col { xs: ColumnSize::Span(12),
                    Card {
                        class: "shadow-sm",
                        header_class: "bg-primary text-white",
                        header: rsx! {
                            h5 { class: "card-title mb-0",
                                Icon { name: "plus-circle", class: "me-2" }
                                "Neues Thema hinzufügen"
                            }
                        },
                        body: rsx! {
                            if add_error().is_some() {
                                Alert {
                                    color: add_error.read().clone().unwrap_or_default().1,
                                    class: "mb-3 d-flex align-items-start",
                                    role: "alert",
                                    Icon { name: "exclamation-circle", class: "me-2 mt-1 flex-shrink-0" }
                                    "{add_error.read().clone().unwrap_or_default().0}"
                                }
                            }
                            Row { class: "g-3 align-items-end",
                                Col { md: ColumnSize::Span(3),
                                    label { class: "form-label", "Thema" }
                                    input {
                                        class: "form-control",
                                        r#type: "text",
                                        placeholder: "Thema Name",
                                        value: "{name}",
                                        oninput: move |e| name.set(e.value()),
                                    }
                                }
                                Col { md: ColumnSize::Span(4),
                                    label { class: "form-label", "E-Mail-Adresse" }
                                    input {
                                        class: "form-control",
                                        r#type: "email",
                                        placeholder: "thema@example.com",
                                        value: "{email_address}",
                                        oninput: move |e| email_address.set(e.value()),
                                    }
                                }
                                Col { md: ColumnSize::Span(3),
                                    label { class: "form-label", "Beschreibung" }
                                    input {
                                        class: "form-control",
                                        r#type: "text",
                                        placeholder: "Kurze Beschreibung",
                                        value: "{description}",
                                        oninput: move |e| description.set(e.value()),
                                    }
                                }
                                Col { md: ColumnSize::Span(2),
                                    label { class: "form-label", "Sichtbarkeit" }
                                    select {
                                        class: "form-select",
                                        onchange: move |e| {
                                            match e.value().as_str() {
                                                "Public" => visibility.set(CategoryVisibility::Public),
                                                "Private" => visibility.set(CategoryVisibility::Private),
                                                _ => {}
                                            }
                                        },
                                        option {
                                            value: "Public",
                                            selected: *visibility.read() == CategoryVisibility::Public,
                                            "Öffentlich"
                                        }
                                        option {
                                            value: "Private",
                                            selected: *visibility.read() == CategoryVisibility::Private,
                                            "Privat"
                                        }
                                    }
                                }
                                Col { md: ColumnSize::Span(1),
                                    Button {
                                        color: Color::Primary,
                                        class: "w-100",
                                        disabled: name.read().is_empty() || email_address.read().is_empty() || *is_sending.read(),
                                        onclick: {
                                            let add = add_invoke.clone();
                                            let mut is_sending = is_sending.clone();
                                            let mut visibility_signal = visibility.clone();
                                            move |_| {
                                                let n = name.read().clone();
                                                let e = email_address.read().clone();
                                                let d = description.read().clone();
                                                let v = visibility_signal.read().clone();
                                                is_sending.set(true);
                                                add(n, e, d, v);
                                            }
                                        },
                                        Icon { name: "plus-lg" }
                                    }
                                }
                            }
                        },
                    }
                }
            }

            // Category table
            Row {
                Col { xs: ColumnSize::Span(12),
                    Card {
                        class: "shadow-sm",
                        header_class: "bg-primary text-white",
                        body_class: "p-0",
                        header: rsx! {
                            h5 { class: "card-title mb-0",
                                Icon { name: "list-ul", class: "me-2" }
                                "Vorhandene Themen"
                                // No Color::White in dioxus-bootstrap-css; keep as raw HTML.
                                span { class: "badge bg-white text-primary ms-2", "{categories().len()}" }
                            }
                        },
                        body: rsx! {
                            if categories().is_empty() {
                                div { class: "p-4 text-muted",
                                    Icon { name: "inbox", class: "me-2" }
                                    "Keine Themen vorhanden."
                                }
                            } else {
                                div { class: "table-responsive",
                                    table { class: "table table-hover mb-0",
                                        thead { class: "table-light",
                                            tr {
                                                th { "Name" }
                                                th { "E-Mail-Adresse" }
                                                th { "Beschreibung" }
                                                th { "Status" }
                                                th { "Sichtbarkeit" }
                                                th { class: "text-end", "Abonnenten" }
                                                th { class: "text-end", "Aktionen" }
                                            }
                                        }
                                        tbody {
                                            for cat in categories() {
                                                {
                                                    let cat_id = cat.id;
                                                    let remove = remove_category.clone();
                                                    let subscriber_count = subscriptions()
                                                        .iter()
                                                        .filter(|s| s.category_id == cat_id && crate::pages::is_active_subscription(&s.status))
                                                        .count();
                                                    rsx! {
                                                        tr {
                                                            style: "cursor: pointer;",
                                                            onclick: move |_| selected_category.set(Some(cat_id)),
                                                            td {
                                                                strong { "{cat.name}" }
                                                            }
                                                            td {
                                                                code { "{cat.email_address}" }
                                                            }
                                                            td { class: "text-muted", "{cat.description}" }
                                                            td {
                                                                if cat.active {
                                                                    Badge { color: Color::Success, "Aktiv" }
                                                                } else {
                                                                    Badge { color: Color::Secondary, "Inaktiv" }
                                                                }
                                                            }
                                                            td {
                                                                if cat.visibility == CategoryVisibility::Public {
                                                                    Badge { color: Color::Info, "Öffentlich" }
                                                                } else {
                                                                    Badge { color: Color::Warning, "Privat" }
                                                                }
                                                            }
                                                            td { class: "text-end",
                                                                Badge { color: Color::Primary, "{subscriber_count}" }
                                                            }
                                                            td { class: "text-end",
                                                                Button {
                                                                    color: Color::Primary,
                                                                    size: Size::Sm,
                                                                    class: "me-1",
                                                                    onclick: move |evt: MouseEvent| {
                                                                        evt.stop_propagation();
                                                                        selected_category.set(Some(cat_id));
                                                                    },
                                                                    Icon { name: "pencil-square", class: "me-1" }
                                                                    "Details"
                                                                }
                                                                Button {
                                                                    color: Color::Danger,
                                                                    size: Size::Sm,
                                                                    onclick: move |evt: MouseEvent| {
                                                                        evt.stop_propagation();
                                                                        info!("Removing category {cat_id}");
                                                                        if let Err(e) = remove(cat_id) {
                                                                            error!("remove_message_category failed: {e:?}");
                                                                        }
                                                                    },
                                                                    Icon { name: "trash", class: "me-1" }
                                                                    "Löschen"
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
            }
        }
    }
}
