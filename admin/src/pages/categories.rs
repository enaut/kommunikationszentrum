use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};

use crate::module_bindings::dioxus::{
    use_reducer_add_message_category, use_reducer_remove_message_category,
    use_table_message_categories,
};

/// Admin-only view: lists all message categories with inline add and delete controls.
#[component]
pub fn CategoriesPage() -> Element {
    let categories = use_table_message_categories();
    let add_category = use_reducer_add_message_category();
    let remove_category = use_reducer_remove_message_category();

    let mut name = use_signal(String::new);
    let mut email_address = use_signal(String::new);
    let mut description = use_signal(String::new);

    rsx! {
        div { class: "container-fluid mt-4",
            div { class: "row mb-3",
                div { class: "col",
                    h2 { class: "mb-0",
                        i { class: "bi bi-tags-fill me-2" }
                        "Kategorien"
                    }
                }
            }

            // Add form
            div { class: "row mb-4",
                div { class: "col-12",
                    div { class: "card shadow-sm",
                        div { class: "card-header bg-primary text-white",
                            h5 { class: "card-title mb-0",
                                i { class: "bi bi-plus-circle me-2" }
                                "Neue Kategorie"
                            }
                        }
                        div { class: "card-body",
                            div { class: "row g-3 align-items-end",
                                div { class: "col-md-3",
                                    label { class: "form-label", "Name" }
                                    input {
                                        class: "form-control",
                                        r#type: "text",
                                        placeholder: "Kategoriename",
                                        value: "{name}",
                                        oninput: move |e| name.set(e.value()),
                                    }
                                }
                                div { class: "col-md-4",
                                    label { class: "form-label", "E-Mail-Adresse" }
                                    input {
                                        class: "form-control",
                                        r#type: "email",
                                        placeholder: "kategorie@example.com",
                                        value: "{email_address}",
                                        oninput: move |e| email_address.set(e.value()),
                                    }
                                }
                                div { class: "col-md-4",
                                    label { class: "form-label", "Beschreibung" }
                                    input {
                                        class: "form-control",
                                        r#type: "text",
                                        placeholder: "Kurze Beschreibung",
                                        value: "{description}",
                                        oninput: move |e| description.set(e.value()),
                                    }
                                }
                                div { class: "col-md-1",
                                    button {
                                        class: "btn btn-primary w-100",
                                        disabled: name.read().is_empty() || email_address.read().is_empty(),
                                        onclick: {
                                            let add = add_category.clone();
                                            move |_| {
                                                let n = name.read().clone();
                                                let e = email_address.read().clone();
                                                let d = description.read().clone();
                                                info!("Adding category: {n}");
                                                if let Err(err) = add(n, e, d) {
                                                    error!("add_message_category failed: {err:?}");
                                                } else {
                                                    name.set(String::new());
                                                    email_address.set(String::new());
                                                    description.set(String::new());
                                                }
                                            }
                                        },
                                        i { class: "bi bi-plus-lg" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Category table
            div { class: "row",
                div { class: "col-12",
                    div { class: "card shadow-sm",
                        div { class: "card-header bg-primary text-white",
                            h5 { class: "card-title mb-0",
                                i { class: "bi bi-list-ul me-2" }
                                "Vorhandene Kategorien"
                                span { class: "badge bg-white text-primary ms-2",
                                    "{categories().len()}"
                                }
                            }
                        }
                        div { class: "card-body p-0",
                            if categories().is_empty() {
                                div { class: "p-4 text-muted",
                                    i { class: "bi bi-inbox me-2" }
                                    "Keine Kategorien vorhanden."
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
                                                th { class: "text-end", "Aktionen" }
                                            }
                                        }
                                        tbody {
                                            for cat in categories() {
                                                {
                                                    let cat_id = cat.id;
                                                    let remove = remove_category.clone();
                                                    rsx! {
                                                        tr {
                                                            td { strong { "{cat.name}" } }
                                                            td { code { "{cat.email_address}" } }
                                                            td { class: "text-muted", "{cat.description}" }
                                                            td {
                                                                if cat.active {
                                                                    span { class: "badge bg-success", "Aktiv" }
                                                                } else {
                                                                    span { class: "badge bg-secondary", "Inaktiv" }
                                                                }
                                                            }
                                                            td { class: "text-end",
                                                                button {
                                                                    class: "btn btn-outline-danger btn-sm",
                                                                    onclick: move |_| {
                                                                        info!("Removing category {cat_id}");
                                                                        if let Err(e) = remove(cat_id) {
                                                                            error!(
                                                                                "remove_message_category failed: {e:?}"
                                                                            );
                                                                        }
                                                                    },
                                                                    i { class: "bi bi-trash me-1" }
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
                        }
                    }
                }
            }
        }
    }
}
