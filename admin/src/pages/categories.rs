use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

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
                                Col { md: ColumnSize::Span(4),
                                    label { class: "form-label", "Beschreibung" }
                                    input {
                                        class: "form-control",
                                        r#type: "text",
                                        placeholder: "Kurze Beschreibung",
                                        value: "{description}",
                                        oninput: move |e| description.set(e.value()),
                                    }
                                }
                                Col { md: ColumnSize::Span(1),
                                    Button {
                                        color: Color::Primary,
                                        class: "w-100",
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
                                                            td { class: "text-end",
                                                                Button {
                                                                    color: Color::Danger,
                                                                    size: Size::Sm,
                                                                    onclick: move |_| {
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
