use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{
    use_procedure_provision_message_category, use_reducer_remove_message_category,
    use_subscription, use_table_visible_domains, use_table_visible_message_categories,
    use_table_visible_subscriptions,
};
use crate::module_bindings::CategoryVisibility;
use crate::pages::category::detail::CategoryDetailPage;

/// Card with form controls to create and provision a new message category / mailing list.
#[component]
pub fn AddCategoryCard() -> Element {
    let domains = use_table_visible_domains();
    let (add_invoke, add_result) = use_procedure_provision_message_category();

    let mut name = use_signal(String::new);
    let mut base = use_signal(String::new);
    let mut selected_domain: Signal<Option<(String, String)>> = use_signal(|| None);
    let mut description = use_signal(String::new);
    let mut visibility = use_signal(|| CategoryVisibility::Public);
    let add_error: Signal<Option<(String, Color)>> = use_signal(|| None);
    let is_sending = use_signal(|| false);

    // React to procedure result signal and update UI accordingly.
    {
        let mut add_result = add_result.clone();
        let mut name = name.clone();
        let mut base = base.clone();
        let mut selected_domain = selected_domain.clone();
        let mut description = description.clone();
        let mut visibility = visibility.clone();
        let mut add_error = add_error.clone();
        let mut is_sending = is_sending.clone();

        use_effect(move || {
            if let Some(res) = add_result() {
                is_sending.set(false);
                match res {
                    Ok(inner) => match inner {
                        Ok(()) => {
                            name.set(String::new());
                            base.set(String::new());
                            selected_domain.set(None);
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

                // Clear the result so the next invocation can be observed
                add_result.set(None);
            }
        });
    }

    let domain_value = selected_domain
        .read()
        .as_ref()
        .map(|(id, name)| format!("{id}:{name}"))
        .unwrap_or_default();
    let visibility_value = match visibility() {
        CategoryVisibility::Public => "Public",
        CategoryVisibility::Private => "Private",
    };

    rsx! {
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
                        Icon { name: "exclamation-circle", class: "me-2 mt-1 flex-shrink-0" }
                        "{add_error.read().clone().unwrap_or_default().0}"
                    }
                }
                Row { class: "g-3 align-items-end",
                    Col { md: ColumnSize::Span(3),
                        FormGroup { label: "Thema",
                            Input {
                                r#type: "text",
                                placeholder: "Thema Name",
                                value: "{name}",
                                oninput: move |e: FormEvent| name.set(e.value()),
                            }
                        }
                    }
                    Col { md: ColumnSize::Span(4),
                        FormGroup { label: "E-Mail-Adresse",
                            InputGroup {
                                Input {
                                    r#type: "text",
                                    placeholder: "postfach",
                                    value: "{base}",
                                    oninput: move |e: FormEvent| base.set(e.value()),
                                }
                                InputGroupText { "@" }
                                Select {
                                    value: domain_value,
                                    onchange: move |e: FormEvent| {
                                        let val = e.value();
                                        if val.is_empty() {
                                            selected_domain.set(None);
                                        } else if let Some(idx) = val.find(':') {
                                            let id = val[..idx].to_string();
                                            let dname = val[idx + 1..].to_string();
                                            selected_domain.set(Some((id, dname)));
                                        }
                                    },
                                    option { value: "", "Domain…" }
                                    for domain in domains() {
                                        option {
                                            key: "{domain.id}",
                                            value: "{domain.id}:{domain.name}",
                                            "{domain.name}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Col { md: ColumnSize::Span(3),
                        FormGroup { label: "Beschreibung",
                            Input {
                                r#type: "text",
                                placeholder: "Kurze Beschreibung",
                                value: "{description}",
                                oninput: move |e: FormEvent| description.set(e.value()),
                            }
                        }
                    }
                    Col { md: ColumnSize::Span(1),
                        FormGroup { label: "Sichtbar",
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
                        }
                    }
                    Col { md: ColumnSize::Span(1),
                        FormGroup { label: " ",
                            Button {
                                color: Color::Primary,
                                class: "w-100",
                                disabled: name.read().is_empty()
                                    || base.read().is_empty()
                                    || selected_domain.read().is_none()
                                    || *is_sending.read(),
                                onclick: {
                                    let add = add_invoke.clone();
                                    let mut is_sending = is_sending.clone();
                                    let visibility_signal = visibility.clone();
                                    move |_| {
                                        let n = name.read().clone();
                                        let b = base.read().clone();
                                        let (domain_id, _) = selected_domain.read().clone().unwrap_or_default();
                                        let d = description.read().clone();
                                        let v = visibility_signal.read().clone();
                                        is_sending.set(true);
                                        add(n, b, domain_id, d, v);
                                    }
                                },
                                Icon { name: "plus-lg" }
                            }
                        }
                    }
                }
            },
        }
    }
}

/// Table displaying all existing message categories.
#[component]
pub fn CategoryTable(mut selected_category: Signal<Option<u64>>) -> Element {
    let categories = use_table_visible_message_categories();
    let subscriptions = use_table_visible_subscriptions();
    let remove_category = use_reducer_remove_message_category();

    rsx! {
        Card {
            class: "shadow-sm",
            header_class: "bg-primary text-white",
            body_class: "p-0",
            header: rsx! {
                h5 { class: "card-title mb-0",
                    Icon { name: "list-ul", class: "me-2" }
                    "Vorhandene Themen"
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
                    Table { hover: true, responsive: true, class: "mb-0",
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
                                            key: "{cat_id}",
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
            },
        }
    }
}

/// Admin-only view: lists all message categories with inline add and delete controls.
#[component]
pub fn CategoriesPage() -> Element {
    use_subscription(&[
        "SELECT * FROM visible_message_categories",
        "SELECT * FROM visible_subscriptions",
        "SELECT * FROM visible_domains",
    ]);

    // When set, the detail/edit page for this category is shown instead of the list.
    let mut selected_category: Signal<Option<u64>> = use_signal(|| None);

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

                Row { class: "mb-4",
                    Col { xs: ColumnSize::Span(12),
                        AddCategoryCard {}
                    }
                }

                Row {
                    Col { xs: ColumnSize::Span(12),
                        CategoryTable { selected_category }
                    }
                }
            }
        }
    }
}
