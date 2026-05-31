use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{
    use_connection_error, use_connection_state, use_reducer_dump_mta_logs_to_server_logs,
    use_reducer_register_admin_identity, use_reducer_unregister_admin_identity,
    use_table_admin_identities, ConnectionState,
};
use crate::oauth::UserInfo;

/// Admin-only view: SpacetimeDB connection details, identity info, and admin identity management.
#[component]
pub fn DebugPage(user_info: UserInfo) -> Element {
    let state = use_connection_state();
    let conn_error = use_connection_error();
    let admin_identities = use_table_admin_identities();
    let register_admin = use_reducer_register_admin_identity();
    let unregister_admin = use_reducer_unregister_admin_identity();
    let dump_logs = use_reducer_dump_mta_logs_to_server_logs();

    let mut register_hex = use_signal(String::new);

    let (alert_color, icon_name, status_text): (Color, &'static str, String) = match state() {
        ConnectionState::Connected(id, _) => (
            Color::Success,
            "check-circle-fill",
            format!("Verbunden · Identity: {id}"),
        ),
        ConnectionState::Connecting => (
            Color::Info,
            "arrow-repeat",
            "Verbindung wird hergestellt…".to_string(),
        ),
        ConnectionState::Reconnecting { attempt, delay_ms } => (
            Color::Warning,
            "exclamation-triangle-fill",
            format!("Wiederverbinden… (Versuch {attempt}, {delay_ms} ms)"),
        ),
        ConnectionState::Error => (
            Color::Danger,
            "exclamation-circle-fill",
            "Verbindungsfehler".to_string(),
        ),
        ConnectionState::Disconnected => (
            Color::Secondary,
            "circle-fill",
            "Nicht verbunden".to_string(),
        ),
    };

    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "bug-fill", class: "me-2" }
                        "Debug & Status"
                    }
                }
            }

            // Connection status card
            Row { class: "mb-4",
                Col { xs: ColumnSize::Span(12),
                    Card {
                        class: "shadow-sm",
                        header_class: "bg-primary text-white",
                        header: rsx! {
                            h5 { class: "card-title mb-0",
                                Icon { name: "plug-fill", class: "me-2" }
                                "SpacetimeDB Verbindung"
                            }
                        },
                        body: rsx! {
                            Alert {
                                color: alert_color,
                                class: "d-flex align-items-start",
                                role: "alert",
                                Icon { name: icon_name, class: "me-2 mt-1 flex-shrink-0" }
                                div { style: "overflow-x: auto; width: 100%;",
                                    div { "{status_text}" }
                                    if let Some(err) = conn_error() {
                                        div { class: "text-danger mt-1 small", "Fehler: {err}" }
                                    }
                                }
                            }
                            Row { class: "text-center",
                                Col { md: ColumnSize::Span(4),
                                    div { class: "border-end",
                                        h6 { class: "text-muted mb-1", "Mitgliedsnummer" }
                                        p { class: "h5 mb-0", "{user_info.mitgliedsnr}" }
                                    }
                                }
                                Col { md: ColumnSize::Span(4),
                                    div { class: "border-end",
                                        h6 { class: "text-muted mb-1", "E-Mail" }
                                        p { class: "h5 mb-0",
                                            if let Some(email) = &user_info.email {
                                                "{email}"
                                            } else {
                                                "–"
                                            }
                                        }
                                    }
                                }
                                Col { md: ColumnSize::Span(4),
                                    div {
                                        h6 { class: "text-muted mb-1", "ID Token" }
                                        p {
                                            style: "font-size: 0.55rem; word-break: break-all;",
                                            if let Some(token) = &user_info.id_token {
                                                "{token}"
                                            } else {
                                                "–"
                                            }
                                        }
                                    }
                                }
                            }
                            div { class: "mt-3",
                                Button {
                                    color: Color::Secondary,
                                    outline: true,
                                    size: Size::Sm,
                                    onclick: move |_| {
                                        info!("Dumping MTA logs to server logs");
                                        if let Err(e) = dump_logs() {
                                            error!("dump_mta_logs_to_server_logs failed: {e:?}");
                                        }
                                    },
                                    Icon { name: "journal-text", class: "me-1" }
                                    "MTA Logs ausgeben"
                                }
                            }
                        }
                    }
                }
            }

            // Admin identity management
            Row {
                Col { xs: ColumnSize::Span(12),
                    Card {
                        class: "shadow-sm",
                        header_class: "bg-primary text-white",
                        header: rsx! {
                            h5 { class: "card-title mb-0",
                                Icon { name: "shield-fill", class: "me-2" }
                                "Admin-Identitäten"
                                span { class: "badge bg-white text-primary ms-2",
                                    "{admin_identities().len()}"
                                }
                            }
                        },
                        body: rsx! {
                            Row { class: "g-2 mb-3",
                                Col {
                                    input {
                                        class: "form-control form-control-sm font-monospace",
                                        r#type: "text",
                                        placeholder: "Identity Hex (64 Zeichen)",
                                        value: "{register_hex}",
                                        oninput: move |e| register_hex.set(e.value()),
                                    }
                                }
                                Col { class: "col-auto",
                                    Button {
                                        color: Color::Primary,
                                        size: Size::Sm,
                                        disabled: register_hex.read().len() != 64,
                                        onclick: {
                                            let register = register_admin.clone();
                                            move |_| {
                                                let hex = register_hex.read().clone();
                                                info!("Registering admin identity: {hex}");
                                                if let Err(e) = register(hex) {
                                                    error!(
                                                        "register_admin_identity failed: {e:?}"
                                                    );
                                                } else {
                                                    register_hex.set(String::new());
                                                }
                                            }
                                        },
                                        Icon { name: "person-plus", class: "me-1" }
                                        "Hinzufügen"
                                    }
                                }
                            }
                            if admin_identities().is_empty() {
                                p { class: "text-muted mb-0",
                                    "Keine Admin-Identitäten registriert."
                                }
                            } else {
                                div { class: "list-group list-group-flush",
                                    for ident in admin_identities() {
                                        {
                                            let hex = ident.identity.to_string();
                                            let hex_for_remove = hex.clone();
                                            let unregister = unregister_admin.clone();
                                            rsx! {
                                                div { class: "list-group-item d-flex justify-content-between align-items-center",
                                                    code { class: "small text-break", "{hex}" }
                                                    Button {
                                                        color: Color::Danger,
                                                        outline: true,
                                                        size: Size::Sm,
                                                        class: "ms-2 flex-shrink-0",
                                                        onclick: move |_| {
                                                            info!(
                                                                "Unregistering admin identity: {hex_for_remove}"
                                                            );
                                                            if let Err(e) =
                                                                unregister(hex_for_remove.clone())
                                                            {
                                                                error!(
                                                                    "unregister_admin_identity failed: {e:?}"
                                                                );
                                                            }
                                                        },
                                                        Icon { name: "person-dash" }
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
