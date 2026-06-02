use ::dioxus::prelude::*;
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{use_table_message_categories, use_table_visible_messages};

// ---------------------------------------------------------------------------
// Visual helpers
// ---------------------------------------------------------------------------

/// Map a category ID to a rotating Bootstrap colour class pair
/// (filled variant for selected, outline variant for unselected).
fn cat_btn_class(category_id: u64, selected: bool) -> &'static str {
    match category_id % 5 {
        0 => {
            if selected {
                "btn btn-sm btn-primary"
            } else {
                "btn btn-sm btn-outline-primary"
            }
        }
        1 => {
            if selected {
                "btn btn-sm btn-success"
            } else {
                "btn btn-sm btn-outline-success"
            }
        }
        2 => {
            if selected {
                "btn btn-sm btn-info"
            } else {
                "btn btn-sm btn-outline-info"
            }
        }
        3 => {
            if selected {
                "btn btn-sm btn-warning"
            } else {
                "btn btn-sm btn-outline-warning"
            }
        }
        _ => {
            if selected {
                "btn btn-sm btn-secondary"
            } else {
                "btn btn-sm btn-outline-secondary"
            }
        }
    }
}

fn cat_badge_color(category_id: u64) -> Color {
    match category_id % 5 {
        0 => Color::Primary,
        1 => Color::Success,
        2 => Color::Info,
        3 => Color::Warning,
        _ => Color::Secondary,
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[component]
pub fn MessagesPage() -> Element {
    let messages = use_table_visible_messages();
    let categories = use_table_message_categories();

    let mut selected_id: Signal<Option<u64>> = use_signal(|| None);
    let mut filter_category: Signal<Option<u64>> = use_signal(|| None);

    // Newest-first
    let mut sorted = messages();
    sorted.sort_by(|a, b| {
        let a_us = a.received_at;
        let b_us = b.received_at;
        b_us.cmp(&a_us)
    });

    // Apply category filter
    let filtered: Vec<_> = sorted
        .into_iter()
        .filter(|m| filter_category().map_or(true, |cat| m.category_id == cat))
        .collect();

    let selected_msg = selected_id().and_then(|id| filtered.iter().find(|m| m.id == id).cloned());

    rsx! {
        Container { fluid: true, class: "mt-4",

            // ── Header ────────────────────────────────────────────────────
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "envelope-fill", class: "me-2" }
                        "Nachrichten"
                    }
                    p { class: "text-muted mt-1",
                        Badge { color: Color::Primary, class: "me-2",
                            "{messages().len()}"
                        }
                        "empfangene Nachrichten"
                    }
                }
            }

            // ── Category filter chips ──────────────────────────────────────
            Row { class: "mb-3",
                Col {
                    div { class: "d-flex flex-wrap gap-2 align-items-center",
                        span { class: "text-muted small me-1", "Filtern:" }
                        button {
                            class: if filter_category().is_none() {
                                "btn btn-sm btn-primary"
                            } else {
                                "btn btn-sm btn-outline-secondary"
                            },
                            onclick: move |_| {
                                filter_category.set(None);
                                selected_id.set(None);
                            },
                            "Alle"
                        }
                        for cat in categories().into_iter().filter(|c| c.active) {
                            {
                                let cat_id = cat.id;
                                let is_active = filter_category() == Some(cat_id);
                                rsx! {
                                    button {
                                        class: cat_btn_class(cat_id, is_active),
                                        onclick: move |_| {
                                            filter_category.set(Some(cat_id));
                                            selected_id.set(None);
                                        },
                                        "{cat.name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Empty state ───────────────────────────────────────────────
            if filtered.is_empty() {
                Alert { color: Color::Info,
                    Icon { name: "inbox", class: "me-2" }
                    if filter_category().is_none() {
                        "Keine Nachrichten vorhanden."
                    } else {
                        "Keine Nachrichten für diese Kategorie."
                    }
                }
            } else {

                // ── Two-column layout ──────────────────────────────────────
                Row {
                    // ── Message list ───────────────────────────────────────
                    Col { md: ColumnSize::Span(4), class: "mb-3",
                        Card {
                            class: "shadow-sm",
                            body_class: "p-0",
                            body: rsx! {
                                div { class: "list-group list-group-flush",
                                    for msg in filtered.clone() {
                                        {
                                            let msg_id = msg.id;
                                            let is_sel = selected_id() == Some(msg_id);
                                            let subject = if msg.subject.is_empty() {
                                                "(kein Betreff)".to_string()
                                            } else {
                                                msg.subject.clone()
                                            };
                                            let sender = msg.from_header.clone();
                                            let cat_email = msg.category_email.clone();
                                            let date_str =
                                                msg.received_at.to_string();
                                            let badge_color = cat_badge_color(msg.category_id);
                                            rsx! {
                                                button {
                                                    class: if is_sel {
                                                        "list-group-item list-group-item-action active px-3 py-2"
                                                    } else {
                                                        "list-group-item list-group-item-action px-3 py-2"
                                                    },
                                                    onclick: move |_| selected_id.set(Some(msg_id)),
                                                    div { class: "d-flex justify-content-between align-items-start mb-1",
                                                        Badge { color: badge_color,
                                                            class: "text-truncate",
                                                            style: "max-width: 10rem;",
                                                            "{cat_email}"
                                                        }
                                                        small {
                                                            class: if is_sel {
                                                                "text-white-50 text-nowrap ms-2"
                                                            } else {
                                                                "text-muted text-nowrap ms-2"
                                                            },
                                                            "{date_str}"
                                                        }
                                                    }
                                                    div { class: "fw-semibold small text-truncate",
                                                        "{subject}"
                                                    }
                                                    div {
                                                        class: if is_sel {
                                                            "small text-white-50 text-truncate"
                                                        } else {
                                                            "small text-muted text-truncate"
                                                        },
                                                        "{sender}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                        }
                    }

                    // ── Detail panel ───────────────────────────────────────
                    Col { md: ColumnSize::Span(8), class: "mb-3",
                        if let Some(msg) = selected_msg {
                            Card {
                                class: "shadow-sm",
                                header: rsx! {
                                    div { class: "d-flex align-items-center gap-2 flex-wrap",
                                        Badge { color: cat_badge_color(msg.category_id),
                                            "{msg.category_email}"
                                        }
                                        span { class: "fw-semibold",
                                            if msg.subject.is_empty() {
                                                "(kein Betreff)"
                                            } else {
                                                "{msg.subject}"
                                            }
                                        }
                                        small { class: "text-muted ms-auto",
                                            {msg.received_at.to_string()}
                                        }
                                    }
                                },
                                body: rsx! {
                                    // ── Parsed header fields ───────────────
                                    table { class: "table table-sm table-borderless mb-0",
                                        tbody {
                                            tr {
                                                th { class: "text-muted small pe-3",
                                                    style: "width: 5.5rem; white-space: nowrap;",
                                                    "Von"
                                                }
                                                td { class: "small", "{msg.from_header}" }
                                            }
                                            tr {
                                                th { class: "text-muted small pe-3", "An" }
                                                td { class: "small", "{msg.category_email}" }
                                            }
                                            if let Some(cc) = &msg.cc_header {
                                                tr {
                                                    th { class: "text-muted small pe-3", "CC" }
                                                    td { class: "small", "{cc}" }
                                                }
                                            }
                                            if let Some(date) = &msg.date_header {
                                                tr {
                                                    th { class: "text-muted small pe-3", "Datum" }
                                                    td { class: "small", "{date}" }
                                                }
                                            }
                                            if let Some(mid) = &msg.message_id {
                                                tr {
                                                    th { class: "text-muted small pe-3", "Message-ID" }
                                                    td { class: "small font-monospace text-break", "{mid}" }
                                                }
                                            }
                                            if let Some(rt) = &msg.reply_to {
                                                tr {
                                                    th { class: "text-muted small pe-3", "Reply-To" }
                                                    td { class: "small", "{rt}" }
                                                }
                                            }
                                        }
                                    }
                                    hr { class: "my-3" }
                                    // ── Body ──────────────────────────────
                                    if msg.body_raw.is_empty() {
                                        Alert { color: Color::Warning, class: "small mb-0",
                                            Icon { name: "exclamation-triangle", class: "me-1" }
                                            "Nachrichteninhalt nicht gespeichert (Nachricht zu groß)."
                                        }
                                    } else {
                                        pre {
                                            class: "small bg-body-secondary rounded p-3 mb-0 overflow-auto",
                                            style: "max-height: 28rem; white-space: pre-wrap; word-break: break-word;",
                                            "{msg.body_raw}"
                                        }
                                    }
                                },
                            }
                        } else {
                            // Placeholder when nothing is selected
                            Card {
                                class: "shadow-sm",
                                body: rsx! {
                                    div { class: "d-flex flex-column align-items-center justify-content-center py-5 text-muted",
                                        Icon { name: "envelope-open", class: "display-6 mb-3" }
                                        p { class: "mb-0", "Nachricht aus der Liste auswählen" }
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
