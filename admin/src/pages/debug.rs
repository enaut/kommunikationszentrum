use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};

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

    let (alert_class, icon_class, status_text) = match state() {
        ConnectionState::Connected(id, _) => (
            "alert-success",
            "bi-check-circle-fill",
            format!("Verbunden · Identity: {id}"),
        ),
        ConnectionState::Connecting => (
            "alert-info",
            "bi-arrow-repeat",
            "Verbindung wird hergestellt…".to_string(),
        ),
        ConnectionState::Reconnecting { attempt, delay_ms } => (
            "alert-warning",
            "bi-exclamation-triangle-fill",
            format!("Wiederverbinden… (Versuch {attempt}, {delay_ms} ms)"),
        ),
        ConnectionState::Error => (
            "alert-danger",
            "bi-exclamation-circle-fill",
            "Verbindungsfehler".to_string(),
        ),
        ConnectionState::Disconnected => (
            "alert-secondary",
            "bi-circle-fill",
            "Nicht verbunden".to_string(),
        ),
    };

    rsx! {
        div { class: "container-fluid mt-4",
            div { class: "row mb-3",
                div { class: "col",
                    h2 { class: "mb-0",
                        i { class: "bi bi-bug-fill me-2" }
                        "Debug & Status"
                    }
                }
            }

            // Connection status card
            div { class: "row mb-4",
                div { class: "col-12",
                    div { class: "card shadow-sm",
                        div { class: "card-header bg-primary text-white",
                            h5 { class: "card-title mb-0",
                                i { class: "bi bi-plug-fill me-2" }
                                "SpacetimeDB Verbindung"
                            }
                        }
                        div { class: "card-body",
                            div {
                                class: "alert {alert_class} d-flex align-items-start",
                                role: "alert",
                                i { class: "bi {icon_class} me-2 mt-1 flex-shrink-0" }
                                div { style: "overflow-x: auto; width: 100%;",
                                    div { "{status_text}" }
                                    if let Some(err) = conn_error() {
                                        div { class: "text-danger mt-1 small", "Fehler: {err}" }
                                    }
                                }
                            }
                            div { class: "row text-center",
                                div { class: "col-md-4",
                                    div { class: "border-end",
                                        h6 { class: "text-muted mb-1", "Mitgliedsnummer" }
                                        p { class: "h5 mb-0", "{user_info.mitgliedsnr}" }
                                    }
                                }
                                div { class: "col-md-4",
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
                                div { class: "col-md-4",
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
                                button {
                                    class: "btn btn-outline-secondary btn-sm",
                                    onclick: move |_| {
                                        info!("Dumping MTA logs to server logs");
                                        if let Err(e) = dump_logs() {
                                            error!("dump_mta_logs_to_server_logs failed: {e:?}");
                                        }
                                    },
                                    i { class: "bi bi-journal-text me-1" }
                                    "MTA Logs ausgeben"
                                }
                            }
                        }
                    }
                }
            }

            // Admin identity management
            div { class: "row",
                div { class: "col-12",
                    div { class: "card shadow-sm",
                        div { class: "card-header bg-primary text-white",
                            h5 { class: "card-title mb-0",
                                i { class: "bi bi-shield-fill me-2" }
                                "Admin-Identitäten"
                                span { class: "badge bg-white text-primary ms-2",
                                    "{admin_identities().len()}"
                                }
                            }
                        }
                        div { class: "card-body",
                            div { class: "row g-2 mb-3",
                                div { class: "col",
                                    input {
                                        class: "form-control form-control-sm font-monospace",
                                        r#type: "text",
                                        placeholder: "Identity Hex (64 Zeichen)",
                                        value: "{register_hex}",
                                        oninput: move |e| register_hex.set(e.value()),
                                    }
                                }
                                div { class: "col-auto",
                                    button {
                                        class: "btn btn-primary btn-sm",
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
                                        i { class: "bi bi-person-plus me-1" }
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
                                                    button {
                                                        class: "btn btn-outline-danger btn-sm ms-2 flex-shrink-0",
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
                                                        i { class: "bi bi-person-dash" }
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
