mod config;
mod module_bindings;
mod oauth;

use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use config::AdminConfig;
use module_bindings::dioxus::{
    use_connection_state, use_reducer_add_message_category, use_spacetimedb_context_provider,
    use_subscription, use_table_visible_accounts, ConnectionState,
};
use module_bindings::*;
use oauth::{use_oauth, AuthState, UserInfo};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const BOOTSTRAP_CSS: Asset = asset!("/assets/static/custom_colors.scss");
const BOOTSTRAP_JS: Asset = asset!("/assets/static/external/bootstrap/bootstrap.bundle.min.js");

fn main() {
    ::dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let config = use_signal(AdminConfig::load);
    let (auth_state, login, logout) = use_oauth(config.read().oauth.clone());

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: BOOTSTRAP_CSS }
        document::Script { src: BOOTSTRAP_JS }
        match &*auth_state.read() {
            AuthState::Unauthenticated => rsx! {
                LoginPage { on_login: login }
            },
            AuthState::Authenticating => rsx! {
                AuthenticatingPage {}
            },
            AuthState::Authenticated(user_info) => rsx! {
                AuthenticatedApp { user_info: user_info.clone(), on_logout: logout }
            },
            AuthState::Error(error) => rsx! {
                ErrorPage { error: error.clone(), on_retry: login }
            },
        }
    }
}

#[component]
fn LoginPage(on_login: EventHandler<()>) -> Element {
    rsx! {
        div { class: "container-fluid vh-100",
            div { class: "row justify-content-center align-items-center h-100",
                div { class: "col-12 col-md-6 col-lg-4",
                    div { class: "card shadow",
                        div { class: "card-body text-center p-5",
                            h1 { class: "card-title h3 mb-4 text-primary", "SolaWis Admin" }
                            p { class: "card-text text-muted mb-4",
                                "Melden Sie sich mit Ihrem SolaWis-Konto an, um auf das Admin-Panel zuzugreifen."
                            }
                            button {
                                class: "btn btn-primary btn-lg w-100",
                                onclick: move |_| on_login.call(()),
                                i { class: "bi bi-box-arrow-in-right me-2" }
                                "Mit SolaWis anmelden"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AuthenticatingPage() -> Element {
    rsx! {
        div { class: "container-fluid vh-100",
            div { class: "row justify-content-center align-items-center h-100",
                div { class: "col-12 col-md-6 col-lg-4",
                    div { class: "card shadow",
                        div { class: "card-body text-center p-5",
                            div {
                                class: "spinner-border text-primary mb-3",
                                role: "status",
                                span { class: "visually-hidden", "Loading..." }
                            }
                            h2 { class: "h4 mb-3", "Authentifizierung läuft..." }
                            p { class: "text-muted", "Sie werden zu SolaWis weitergeleitet..." }
                            div { class: "progress mt-4",
                                div {
                                    class: "progress-bar progress-bar-striped progress-bar-animated",
                                    role: "progressbar",
                                    style: "width: 100%",
                                    "aria-valuenow": "100",
                                    "aria-valuemin": "0",
                                    "aria-valuemax": "100",
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AuthenticatedApp(user_info: UserInfo, on_logout: EventHandler<()>) -> Element {
    let config = use_signal(AdminConfig::load);
    let uri = config.read().spacetimedb_uri.clone();
    let module_name = config.read().spacetimedb_module_name.clone();

    info!("Authenticated as: {}", user_info.mitgliedsnr);

    // Establish the SpacetimeDB connection and provide context to all children.
    // The OAuth id_token is passed so SpacetimeDB can verify the caller's identity.
    let _ctx = use_spacetimedb_context_provider(&uri, &module_name, user_info.id_token.clone());

    // Subscribe here so data is available to all child components (e.g. ConnectionStatusCard).
    use_subscription(&[
        "SELECT * FROM visible_accounts",
        "SELECT * FROM admin_identities",
    ]);

    let state = use_connection_state();

    rsx! {
        // Bootstrap Navbar
        nav { class: "navbar navbar-expand-lg navbar-dark bg-primary",
            div { class: "container-fluid",
                a { class: "navbar-brand", href: "#",
                    i { class: "bi bi-gear-fill me-2" }
                    "SolaWis Admin"
                }
                button {
                    class: "navbar-toggler",
                    "type": "button",
                    "data-bs-toggle": "collapse",
                    "data-bs-target": "#navbarNav",
                    span { class: "navbar-toggler-icon" }
                }
                div { class: "collapse navbar-collapse", id: "navbarNav",
                    ul { class: "navbar-nav ms-auto",
                        li { class: "nav-item dropdown",
                            a {
                                class: "nav-link dropdown-toggle",
                                href: "#",
                                role: "button",
                                "data-bs-toggle": "dropdown",
                                i { class: "bi bi-person-circle me-2" }
                                "{user_info.mitgliedsnr}"
                            }
                            ul { class: "dropdown-menu",
                                li {
                                    a { class: "dropdown-item", href: "#",
                                        i { class: "bi bi-person me-2" }
                                        "Profil"
                                    }
                                }
                                li {
                                    hr { class: "dropdown-divider" }
                                }
                                li {
                                    button {
                                        class: "dropdown-item",
                                        onclick: move |_| on_logout.call(()),
                                        i { class: "bi bi-box-arrow-right me-2" }
                                        "Abmelden"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Main Content
        div { class: "container-fluid mt-4",
            div { class: "row",
                div { class: "col-12",
                    ConnectionStatusCard { user_info: user_info.clone() }
                }
            }
            match state() {
                ConnectionState::Connected(_, _) => rsx! {
                    div { class: "row mt-4",
                        div { class: "col-12", AccountsSection {} }
                    }
                },
                _ => rsx! {},
            }
        }
    }
}

#[component]
fn ErrorPage(error: String, on_retry: Callback<()>) -> Element {
    rsx! {
        section { class: "hero is-fullheight is-danger",
            div { class: "hero-body",
                div { class: "container has-text-centered",
                    div { class: "columns is-centered",
                        div { class: "column is-6",
                            div { class: "box",
                                div { class: "block",
                                    span { class: "icon is-large has-text-danger",
                                        i { class: "fas fa-exclamation-triangle fa-2x" }
                                    }
                                }
                                h1 { class: "title is-4 has-text-danger", "Authentifizierungsfehler" }
                                div { class: "content",
                                    p { class: "has-text-grey", "{error}" }
                                }
                                button {
                                    class: "button is-primary is-medium",
                                    onclick: move |_| on_retry.call(()),
                                    span { class: "icon",
                                        i { class: "fas fa-redo" }
                                    }
                                    span { "Erneut versuchen" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Displays the SpacetimeDB connection status and a summary of key counts.
/// Reads connection state and table data directly from context — no props needed.
#[component]
fn ConnectionStatusCard(user_info: UserInfo) -> Element {
    let state = use_connection_state();
    let accounts = use_table_visible_accounts();

    let (alert_class, icon, identity_str) = match state() {
        ConnectionState::Connected(id, _) => {
            ("alert-success", "bi-check-circle-fill", format!("{id:?}"))
        }
        ConnectionState::Connecting => (
            "alert-info",
            "bi-arrow-repeat",
            "Verbindung wird hergestellt…".to_string(),
        ),
        ConnectionState::Reconnecting { attempt, delay_ms } => (
            "alert-warning",
            "bi-exclamation-triangle-fill",
            format!("Wiederverbinden… (Versuch {attempt}, {delay_ms}ms)"),
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
        div { class: "card shadow-sm",
            div { class: "card-header bg-primary text-white",
                h5 { class: "card-title mb-0",
                    i { class: "bi bi-info-circle-fill me-2" }
                    "SpacetimeDB Status"
                }
            }
            div { class: "card-body",
                div {
                    class: "alert {alert_class} d-flex align-items-center",
                    role: "alert",
                    style: "overflow-x: auto;",
                    i { class: "bi {icon} me-2" }
                    div {
                        "Verbunden als: "
                        strong { "{user_info.mitgliedsnr}" }
                        pre { "Identity: {user_info.decode_id_token():#?}" }
                        div { "SpacetimeDB Identity: {identity_str}" }
                    }
                }
                div { class: "row text-center",
                    div { class: "col-md-4",
                        div { class: "border-end",
                            h6 { class: "text-muted mb-1", "Benutzer" }
                            p { class: "h5 mb-0",
                                if let Some(name) = &user_info.name {
                                    "{name}"
                                } else {
                                    "{user_info.username}"
                                }
                            }
                        }
                    }
                    div { class: "col-md-4",
                        div { class: "border-end",
                            h6 { class: "text-muted mb-1", "Mitgliedsnummer" }
                            p { class: "h5 mb-0", "{user_info.mitgliedsnr}" }
                            p { style: "font-size: 0.6rem;",
                                "{user_info.id_token.as_ref().unwrap_or(&String::new())}"
                            }
                        }
                    }
                    div { class: "col-md-4",
                        div {
                            h6 { class: "text-muted mb-1", "Accounts" }
                            p { class: "h5 mb-0",
                                span { class: "badge bg-primary", "{accounts().len()}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Lists all visible accounts and lets admins add a message category per account.
/// Self-contained: subscribes to its own table, reads via hook, invokes reducer via hook.
#[component]
fn AccountsSection() -> Element {
    let accounts = use_table_visible_accounts();
    let add_message_category = use_reducer_add_message_category();

    rsx! {
        div { class: "card shadow-sm",
            div { class: "card-header bg-primary text-white",
                h5 { class: "card-title mb-0",
                    i { class: "bi bi-people-fill me-2" }
                    "SoLaWi Mitglieder"
                }
            }
            div { class: "card-body",
                if accounts().is_empty() {
                    div {
                        class: "alert alert-info d-flex align-items-center",
                        role: "alert",
                        i { class: "bi bi-info-circle me-2" }
                        "Keine Accounts gefunden. Verbindung zur Datenbank prüfen."
                    }
                } else {
                    div { class: "row",
                        for user in accounts() {
                            div { class: "col-md-6 col-lg-4 mb-3",
                                div { class: "card h-100 border-primary",
                                    div { class: "card-body",
                                        div { class: "d-flex align-items-center mb-3",
                                            i {
                                                class: "bi bi-person-circle text-primary me-2",
                                                style: "font-size: 2rem;",
                                            }
                                            div {
                                                h6 { class: "card-title mb-0", "{user.name}" }
                                                ul {
                                                    li {
                                                        small { class: "text-muted", "{user.email}" }
                                                    }
                                                    li {
                                                        small { class: "text-muted", "{user.identity}" }
                                                    }
                                                }
                                            }
                                        }
                                        div { class: "mb-3",
                                            span { class: "badge bg-secondary me-1",
                                                "ID: {user.id}"
                                            }
                                            if user.is_active {
                                                span { class: "badge bg-success", "Aktiv" }
                                            } else {
                                                span { class: "badge bg-danger", "Inaktiv" }
                                            }
                                        }
                                        button {
                                            class: "btn btn-primary btn-sm w-100",
                                            onclick: {
                                                let user_name = user.name.clone();
                                                let user_email = user.email.clone();
                                                let add_message_category = add_message_category.clone();
                                                move |_| {
                                                    info!("Adding message category for: {} ({})", user_name, user_email);
                                                    if let Err(e) = add_message_category(
                                                        user_name.clone(),
                                                        user_email.clone(),
                                                        "Standard Kategorie".to_string(),
                                                    ) {
                                                        error!("add_message_category failed for {}: {e:?}", user_name);
                                                    }
                                                }
                                            },
                                            i { class: "bi bi-plus-lg me-1" }
                                            "Kategorie hinzufügen"
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
