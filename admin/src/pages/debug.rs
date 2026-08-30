use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{
    use_connection_error, use_connection_state, use_procedure_sync_stalwart_domains,
    use_reducer_cancel_mail_delivery_retry, use_reducer_create_webhook_token,
    use_reducer_dump_mta_logs_to_server_logs, use_reducer_register_admin_identity,
    use_reducer_revoke_webhook_token, use_reducer_unregister_admin_identity, use_subscription,
    use_table_sender_mail_delivery_claimed, use_table_sender_mail_delivery_done,
    use_table_sender_mail_delivery_events, use_table_sender_mail_delivery_pending,
    use_table_sender_mail_delivery_temporary_failed, use_table_visible_admin_identities,
    use_table_visible_domains, use_table_visible_webhook_tokens, ConnectionState,
};
use crate::oauth::UserInfo;
use wasm_bindgen_futures::{spawn_local, JsFuture};

#[derive(Clone, Copy, PartialEq, Eq)]
enum DebugSection {
    Configuration,
    DbConnection,
    TemporaryFailed,
    MailDeliveryEvents,
    MailDeliveryDone,
    MailDeliveryPending,
    MailDeliveryClaimed,
}

/// Admin-only view: SpacetimeDB connection details, identity info, and admin identity management.
#[component]
pub fn DebugPage(user_info: UserInfo) -> Element {
    use_subscription(&[
        "SELECT * FROM visible_admin_identities",
        "SELECT * FROM visible_webhook_tokens",
        "SELECT * FROM visible_domains",
        "SELECT * FROM sender_mail_delivery_temporary_failed",
        "SELECT * FROM sender_mail_delivery_events",
    ]);
    let state = use_connection_state();
    let conn_error = use_connection_error();
    let admin_identities = use_table_visible_admin_identities();
    let domains = use_table_visible_domains();
    let register_admin = use_reducer_register_admin_identity();
    let unregister_admin = use_reducer_unregister_admin_identity();
    let dump_logs = use_reducer_dump_mta_logs_to_server_logs();
    let create_webhook_token = use_reducer_create_webhook_token();
    let revoke_webhook_token = use_reducer_revoke_webhook_token();
    let (sync_domains_invoke, sync_domains_result) = use_procedure_sync_stalwart_domains();

    let admin_tokens = use_table_visible_webhook_tokens();
    let temporary_failed = use_table_sender_mail_delivery_temporary_failed();
    let pending_deliveries = use_table_sender_mail_delivery_pending();
    let claimed_deliveries = use_table_sender_mail_delivery_claimed();
    let done_deliveries = use_table_sender_mail_delivery_done();
    let delivery_events = use_table_sender_mail_delivery_events();
    let cancel_retry = use_reducer_cancel_mail_delivery_retry();
    let mut administration_menu_open = use_signal(|| false);
    let mut selected_section = use_signal(|| DebugSection::Configuration);

    let mut register_hex = use_signal(String::new);
    let mut token_plain = use_signal(String::new);
    let mut token_hash = use_signal(String::new);
    let mut token_label = use_signal(String::new);
    let mut token_copy_button_label = use_signal(|| "Copy".to_string());
    let mut permissions_input = use_signal(String::new);

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

            Row { class: "mb-3",
                Col { xs: ColumnSize::Span(12),
                    div { class: "dropdown d-inline-block",
                        button {
                            class: "btn btn-outline-primary dropdown-toggle",
                            r#type: "button",
                            aria_expanded: if administration_menu_open() { "true" } else { "false" },
                            onclick: move |_| administration_menu_open.set(!administration_menu_open()),
                            "Verwaltung"
                        }
                        if administration_menu_open() {
                            div { class: "dropdown-menu show shadow-sm",
                                div { class: "dropdown-header fw-semibold", "Konfiguration" }
                                button {
                                    class: "dropdown-item",
                                    onclick: move |_| {
                                        selected_section.set(DebugSection::Configuration);
                                        administration_menu_open.set(false);
                                    },
                                    "Admin, Tokens, Domains"
                                }
                                div { class: "dropdown-divider" }
                                div { class: "dropdown-header fw-semibold", "Datenbank" }
                                button {
                                    class: "dropdown-item",
                                    onclick: move |_| {
                                        selected_section.set(DebugSection::DbConnection);
                                        administration_menu_open.set(false);
                                    },
                                    "DB Connection"
                                }
                                button {
                                    class: "dropdown-item",
                                    onclick: move |_| {
                                        selected_section.set(DebugSection::TemporaryFailed);
                                        administration_menu_open.set(false);
                                    },
                                    "mail_delivery_temporary_failed"
                                }
                                button {
                                    class: "dropdown-item",
                                    onclick: move |_| {
                                        selected_section.set(DebugSection::MailDeliveryEvents);
                                        administration_menu_open.set(false);
                                    },
                                    "mail_delivery_events"
                                }
                                button {
                                    class: "dropdown-item",
                                    onclick: move |_| {
                                        selected_section.set(DebugSection::MailDeliveryDone);
                                        administration_menu_open.set(false);
                                    },
                                    "mail_delivery_done"
                                }
                                button {
                                    class: "dropdown-item",
                                    onclick: move |_| {
                                        selected_section.set(DebugSection::MailDeliveryPending);
                                        administration_menu_open.set(false);
                                    },
                                    "mail_delivery_pending"
                                }
                                button {
                                    class: "dropdown-item",
                                    onclick: move |_| {
                                        selected_section.set(DebugSection::MailDeliveryClaimed);
                                        administration_menu_open.set(false);
                                    },
                                    "mail_delivery_claimed"
                                }
                            }
                        }
                    }
                }
            }

            if matches!(selected_section(), DebugSection::DbConnection) {
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
            }

            if matches!(selected_section(), DebugSection::Configuration) {
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
                                                        error!("register_admin_identity failed: {e:?}");
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
                                    p { class: "text-muted mb-0", "Keine Admin-Identitäten registriert." }
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
                                                                info!("Unregistering admin identity: {hex_for_remove}");
                                                                if let Err(e) = unregister(hex_for_remove.clone()) {
                                                                    error!("unregister_admin_identity failed: {e:?}");
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

                Row { class: "mt-4",
                    Col { xs: ColumnSize::Span(12),
                        Card {
                            class: "shadow-sm",
                            header_class: "bg-primary text-white",
                            header: rsx! {
                                h5 { class: "card-title mb-0",
                                    Icon { name: "key-fill", class: "me-2" }
                                    "Webhook Tokens"
                                }
                            },
                            body: rsx! {
                                Row { class: "g-2 mb-3",
                                    Col {
                                        input {
                                            class: "form-control form-control-sm",
                                            r#type: "text",
                                            placeholder: "Label",
                                            value: "{token_label}",
                                            oninput: move |e| token_label.set(e.value()),
                                        }
                                    }
                                    Col {
                                        input {
                                            class: "form-control form-control-sm",
                                            r#type: "text",
                                            placeholder: "Permissions (comma-separated, e.g. mta-hook,sync-user)",
                                            value: "{permissions_input}",
                                            oninput: move |e| permissions_input.set(e.value()),
                                        }
                                    }
                                    Col { class: "col-auto",
                                        Button {
                                            color: Color::Primary,
                                            size: Size::Sm,
                                            onclick: move |_| {
                                                let mut bytes = [0u8; 32];
                                                if getrandom::fill(&mut bytes).is_err() {
                                                    error!("Failed to generate secure random bytes");
                                                    return;
                                                }
                                                let token = hex::encode(bytes);
                                                token_plain.set(token.clone());
                                                let hash = hex::encode(blake3::hash(token.as_bytes()).as_bytes());
                                                token_hash.set(hash);
                                            },
                                            Icon { name: "plus", class: "me-1" }
                                            "Generate Token"
                                        }
                                    }
                                }

                                if token_plain.read().len() > 0 {
                                    div { class: "mb-2 d-flex align-items-start",
                                        code { class: "small text-break flex-grow-1", "{token_plain}" }
                                        Button { color: Color::Secondary, outline: true, size: Size::Sm, class: "ms-2 flex-shrink-0",
                                            onclick: move |_| {
                                                let token_to_copy = token_plain.read().clone();
                                                spawn_local(async move {
                                                    if let Some(window) = web_sys::window() {
                                                        let promise = window.navigator().clipboard().write_text(&token_to_copy);
                                                        let ret = JsFuture::from(promise).await;
                                                        match ret {
                                                            Ok(_) => {
                                                                token_copy_button_label.set("Copied!".to_string());
                                                                info!("Token copied to clipboard")
                                                            },
                                                            Err(e) => {
                                                                token_copy_button_label.set("Failed to Copy!".to_string());
                                                                error!("Failed to copy token to clipboard: {e:?}")
                                                            },
                                                        }
                                                    } else {
                                                        error!("No window object available to access clipboard");
                                                    }
                                                });
                                            },
                                            Icon { name: "clipboard", class: "me-1" }
                                            {token_copy_button_label}
                                        }
                                    }
                                }

                                Button {
                                    color: Color::Success,
                                    size: Size::Sm,
                                    disabled: token_hash.read().is_empty() || token_label.read().is_empty(),
                                    onclick: {
                                        let create = create_webhook_token.clone();
                                        let label_clone = token_label.read().clone();
                                        let perms = permissions_input.read().clone();
                                        move |_| {
                                            let perms_vec = perms
                                                .split(',')
                                                .map(|s| s.trim().to_string())
                                                .filter(|s| !s.is_empty())
                                                .collect::<Vec<_>>();
                                            let hash = token_hash.read().clone();
                                            info!("Creating webhook token with label: {}", label_clone);
                                            if let Err(e) = create(hash, label_clone.clone(), perms_vec) {
                                                error!("create_webhook_token failed: {e:?}");
                                            } else {
                                                token_plain.set(String::new());
                                                token_hash.set(String::new());
                                                token_label.set(String::new());
                                                permissions_input.set(String::new());
                                            }
                                        }
                                    },
                                    Icon { name: "key", class: "me-1" }
                                    "Create Token"
                                }

                                if !admin_tokens().is_empty() {
                                    div { class: "list-group list-group-flush mt-3",
                                        for t in admin_tokens() {
                                            {
                                                let hash = t.token_hash.clone();
                                                let label = t.label.clone();
                                                let perms = t.permissions.join(", ");
                                                let revoke = revoke_webhook_token.clone();
                                                rsx! {
                                                    div { class: "list-group-item d-flex justify-content-between align-items-start",
                                                        div {
                                                            code { class: "small text-break", "{hash}" }
                                                            div { class: "small text-muted", "{label} · {perms}" }
                                                        }
                                                        Button {
                                                            color: Color::Danger,
                                                            outline: true,
                                                            size: Size::Sm,
                                                            class: "ms-2 flex-shrink-0",
                                                            onclick: move |_| {
                                                                info!("Revoking webhook token: {}", hash);
                                                                if let Err(e) = revoke(hash.clone()) {
                                                                    error!("revoke_webhook_token failed: {e:?}");
                                                                }
                                                            },
                                                            Icon { name: "trash" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    p { class: "text-muted mb-0", "Keine Webhook Tokens erstellt." }
                                }

                                p { class: "small text-muted mt-2", "The token plaintext is shown only once in the browser and is not sent to the server. The server stores only a BLAKE3 hash." }
                            }
                        }
                    }
                }

                Row { class: "mt-4",
                    Col { xs: ColumnSize::Span(12),
                        Card {
                            class: "shadow-sm",
                            header_class: "bg-primary text-white",
                            header: rsx! {
                                h5 { class: "card-title mb-0 d-flex justify-content-between align-items-center",
                                    span {
                                        Icon { name: "globe", class: "me-2" }
                                        "Domains"
                                        span { class: "badge bg-white text-primary ms-2", "{domains().len()}" }
                                    }
                                    Button {
                                        color: Color::Light,
                                        size: Size::Sm,
                                        class: "ms-2",
                                        onclick: {
                                            let sync = sync_domains_invoke.clone();
                                            move |_| {
                                                info!("Triggering sync_stalwart_domains");
                                                sync();
                                            }
                                        },
                                        Icon { name: "arrow-repeat", class: "me-1" }
                                        "Jetzt synchronisieren"
                                    }
                                }
                            },
                            body: rsx! {
                                if let Some(result) = sync_domains_result() {
                                    match result {
                                        Ok(Ok(r)) => rsx! {
                                            Alert {
                                                color: Color::Success,
                                                class: "mb-3 d-flex align-items-start",
                                                Icon { name: "check-circle", class: "me-2 mt-1 flex-shrink-0" }
                                                span {
                                                    "Synchronisierung abgeschlossen: "
                                                    strong { "{r.domains_found}" } " gefunden, "
                                                    strong { "{r.domains_added}" } " hinzugefügt, "
                                                    strong { "{r.domains_updated}" } " aktualisiert, "
                                                    strong { "{r.domains_removed}" } " entfernt."
                                                }
                                            }
                                        },
                                        Ok(Err(proc_err)) => rsx! {
                                            Alert {
                                                color: Color::Danger,
                                                class: "mb-3 d-flex align-items-start",
                                                Icon { name: "exclamation-circle", class: "me-2 mt-1 flex-shrink-0" }
                                                "{proc_err}"
                                            }
                                        },
                                        Err(internal_err) => rsx! {
                                            Alert {
                                                color: Color::Danger,
                                                class: "mb-3 d-flex align-items-start",
                                                Icon { name: "exclamation-circle", class: "me-2 mt-1 flex-shrink-0" }
                                                "Interner Fehler: {internal_err}"
                                            }
                                        },
                                    }
                                }
                                if domains().is_empty() {
                                    p { class: "text-muted mb-0",
                                        Icon { name: "inbox", class: "me-2" }
                                        "Keine Domains vorhanden. Bitte synchronisieren."
                                    }
                                } else {
                                    div { class: "table-responsive",
                                        table { class: "table table-hover mb-0",
                                            thead { class: "table-light",
                                                tr {
                                                    th { "ID" }
                                                    th { "Name" }
                                                    th { "Beschreibung" }
                                                }
                                            }
                                            tbody {
                                                for domain in domains() {
                                                    tr {
                                                        td { code { "{domain.id}" } }
                                                        td { strong { "{domain.name}" } }
                                                        td { class: "text-muted",
                                                            if let Some(ref desc) = domain.description {
                                                                "{desc}"
                                                            } else {
                                                                "–"
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

            if matches!(selected_section(), DebugSection::TemporaryFailed) {
                Row { class: "mt-4",
                    Col { xs: ColumnSize::Span(12),
                        Card {
                            class: "shadow-sm",
                            header_class: "bg-warning text-dark",
                            header: rsx! {
                                h5 { class: "card-title mb-0",
                                    Icon { name: "clock-history", class: "me-2" }
                                    "mail_delivery_temporary_failed"
                                    span { class: "badge bg-dark text-white ms-2",
                                        "{temporary_failed().len()}"
                                    }
                                }
                            },
                            body: rsx! {
                                if temporary_failed().is_empty() {
                                    p { class: "text-muted mb-0",
                                        Icon { name: "inbox", class: "me-2" }
                                        "Keine temporär fehlgeschlagenen Mail-Deliveries."
                                    }
                                } else {
                                    div { class: "table-responsive",
                                        table { class: "table table-hover table-sm mb-0 align-middle",
                                            thead { class: "table-light",
                                                tr {
                                                    th { "Delivery ID" }
                                                    th { "Recipient" }
                                                    th { "Retry at" }
                                                    th { "Reason" }
                                                    th { class: "text-end", "Action" }
                                                }
                                            }
                                            tbody {
                                                for failed in temporary_failed() {
                                                    {
                                                        let delivery_id = failed.id.clone();
                                                        let cancel = cancel_retry.clone();
                                                        rsx! {
                                                            tr {
                                                                td { code { class: "small text-break", "{failed.id}" } }
                                                                td { small { class: "text-muted", "{failed.row.recipient_email}" } }
                                                                td { small { class: "text-muted", "{failed.next_attempt_at}" } }
                                                                td {
                                                                    div { class: "small text-break", "{failed.fail_reason}" }
                                                                }
                                                                td { class: "text-end",
                                                                    Button {
                                                                        color: Color::Warning,
                                                                        outline: true,
                                                                        size: Size::Sm,
                                                                        onclick: move |_| {
                                                                            if let Err(e) = cancel(delivery_id.clone()) {
                                                                                error!("cancel_mail_delivery_retry failed: {e:?}");
                                                                            }
                                                                        },
                                                                        Icon { name: "x-circle", class: "me-1" }
                                                                        "Cancel"
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

            if matches!(selected_section(), DebugSection::MailDeliveryPending) {
                Row { class: "mt-4",
                    Col { xs: ColumnSize::Span(12),
                        Card {
                            class: "shadow-sm",
                            header_class: "bg-secondary text-white",
                            header: rsx! {
                                h5 { class: "card-title mb-0",
                                    Icon { name: "hourglass-split", class: "me-2" }
                                    "mail_delivery_pending"
                                    span { class: "badge bg-white text-secondary ms-2",
                                        "{pending_deliveries().len()}"
                                    }
                                }
                            },
                            body: rsx! {
                                if pending_deliveries().is_empty() {
                                    p { class: "text-muted mb-0", "Keine offenen Deliveries." }
                                } else {
                                    div { class: "table-responsive",
                                        table { class: "table table-sm table-hover mb-0",
                                            thead { class: "table-light",
                                                tr { th { "ID" } th { "Recipient" } th { "Ingress" } }
                                            }
                                            tbody {
                                                for row in pending_deliveries() {
                                                    tr {
                                                        td { code { class: "small text-break", "{row.id}" } }
                                                        td { small { class: "text-muted", "{row.row.recipient_email}" } }
                                                        td { small { class: "text-muted", "{row.ingress_id}" } }
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

            if matches!(selected_section(), DebugSection::MailDeliveryClaimed) {
                Row { class: "mt-4",
                    Col { xs: ColumnSize::Span(12),
                        Card {
                            class: "shadow-sm",
                            header_class: "bg-info text-white",
                            header: rsx! {
                                h5 { class: "card-title mb-0",
                                    Icon { name: "person-workspace", class: "me-2" }
                                    "mail_delivery_claimed"
                                    span { class: "badge bg-white text-info ms-2",
                                        "{claimed_deliveries().len()}"
                                    }
                                }
                            },
                            body: rsx! {
                                if claimed_deliveries().is_empty() {
                                    p { class: "text-muted mb-0", "Keine geclaimten Deliveries." }
                                } else {
                                    div { class: "table-responsive",
                                        table { class: "table table-sm table-hover mb-0",
                                            thead { class: "table-light",
                                                tr { th { "ID" } th { "Worker" } th { "Lease" } th { "Recipient" } }
                                            }
                                            tbody {
                                                for row in claimed_deliveries() {
                                                    tr {
                                                        td { code { class: "small text-break", "{row.id}" } }
                                                        td { small { class: "text-muted", "{row.worker}" } }
                                                        td { small { class: "text-muted", "{row.lease_expires_at}" } }
                                                        td { small { class: "text-muted", "{row.row.recipient_email}" } }
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

            if matches!(selected_section(), DebugSection::MailDeliveryDone) {
                Row { class: "mt-4",
                    Col { xs: ColumnSize::Span(12),
                        Card {
                            class: "shadow-sm",
                            header_class: "bg-success text-white",
                            header: rsx! {
                                h5 { class: "card-title mb-0",
                                    Icon { name: "check-circle", class: "me-2" }
                                    "mail_delivery_done"
                                    span { class: "badge bg-white text-success ms-2",
                                        "{done_deliveries().len()}"
                                    }
                                }
                            },
                            body: rsx! {
                                if done_deliveries().is_empty() {
                                    p { class: "text-muted mb-0", "Keine abgeschlossenen Deliveries." }
                                } else {
                                    div { class: "table-responsive",
                                        table { class: "table table-sm table-hover mb-0",
                                            thead { class: "table-light",
                                                tr { th { "ID" } th { "Final State" } th { "Recipient" } }
                                            }
                                            tbody {
                                                for row in done_deliveries() {
                                                    tr {
                                                        td { code { class: "small text-break", "{row.id}" } }
                                                        td { small { class: "text-muted", "{row.final_state}" } }
                                                        td { small { class: "text-muted", "{row.row.recipient_email}" } }
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

            if matches!(selected_section(), DebugSection::MailDeliveryEvents) {
                Row { class: "mt-4",
                    Col { xs: ColumnSize::Span(12),
                        Card {
                            class: "shadow-sm",
                            header_class: "bg-dark text-white",
                            header: rsx! {
                                h5 { class: "card-title mb-0",
                                    Icon { name: "journal-text", class: "me-2" }
                                    "mail_delivery_events"
                                }
                            },
                            body: rsx! {
                                if delivery_events().is_empty() {
                                    p { class: "text-muted mb-0", "Keine Mail-Delivery-Events." }
                                } else {
                                    div { class: "table-responsive",
                                        table { class: "table table-sm table-hover mb-0",
                                            thead { class: "table-light",
                                                tr { th { "ID" } th { "Type" } th { "Attempt" } th { "Details" } }
                                            }
                                            tbody {
                                                for event in delivery_events() {
                                                    tr {
                                                        td { code { class: "small text-break", "{event.id}" } }
                                                        td { small { class: "text-muted", "{event.event_type}" } }
                                                        td { small { class: "text-muted", "{event.attempt_no}" } }
                                                        td { div { class: "small text-break", "{event.details}" } }
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
