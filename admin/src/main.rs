mod config;
mod module_bindings;
mod oauth;
mod use_spacetime_db;

use config::AdminConfig;
use dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use module_bindings::*;
use oauth::{use_oauth, AuthState, UserInfo};
use spacetimedb_sdk::Identity;
use use_spacetime_db::{use_accounts_table, use_spacetime_db, ConnectionState, SpacetimeDbOptions};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const BOOTSTRAP_CSS: Asset = asset!("/assets/static/custom_colors.scss");
const BOOTSTRAP_JS: Asset = asset!("/assets/static/external/bootstrap/bootstrap.bundle.min.js");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Load configuration
    let config = use_signal(AdminConfig::load);
    
    // OAuth authentication hook
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
    // Load configuration
    let config = use_signal(AdminConfig::load);
    let config_read = config.read();
    
    // SpacetimeDB connection with authentication token
    info!("Authenticated as: {}", user_info.mitgliedsnr);
    if user_info.id_token.is_some() {
        info!("Using SpacetimeDB with id_token present");
    } else {
        info!("No id_token present; will try access_token as fallback");
    }
    // Keep token in a signal so we can trigger reconnects if it changes (e.g., on refresh)
    let id_token_sig = use_signal(|| user_info.id_token.clone());

    let spacetime_db = use_spacetime_db(SpacetimeDbOptions {
        uri: config_read.spacetimedb_uri.clone(),
        module_name: config_read.spacetimedb_module_name.clone(),
        token: id_token_sig.read().clone(),
    });
    let _subsc = use_spacetime_db::use_spacetime_subscription(
        &spacetime_db,
        vec!["SELECT * FROM account".to_string()],
    );

    let accounts = use_accounts_table(&spacetime_db);

    // Provide SpacetimeDB context for child components
    use_context_provider(|| spacetime_db.clone());

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
                    ConnectionStatusCard {
                        status: "info",
                        title: "SpacetimeDB Status",
                        user_info: user_info.clone(),
                        identity: spacetime_db.identity.read().unwrap_or_else(Identity::__dummy),
                        accounts_count: accounts.read().len(),
                    }
                }
            }
            match &*spacetime_db.state.read() {
                ConnectionState::Connected(_) => rsx! {
                    div { class: "row mt-4",
                        div { class: "col-12",
                            AccountsSection { accounts }
                        }
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

#[component]
fn ConnectionStatusCard(
    status: &'static str,
    title: &'static str,
    user_info: UserInfo,
    identity: Identity,
    accounts_count: usize,
) -> Element {
    let (alert_class, icon_class, icon) = match status {
        "success" => ("alert-success", "text-success", "bi-check-circle-fill"),
        "error" => ("alert-danger", "text-danger", "bi-exclamation-circle-fill"),
        "warning" => (
            "alert-warning",
            "text-warning",
            "bi-exclamation-triangle-fill",
        ),
        "info" => ("alert-info", "text-info", "bi-info-circle-fill"),
        _ => ("alert-secondary", "text-secondary", "bi-circle-fill"),
    };

    rsx! {
        div { class: "card shadow-sm",
            div { class: "card-header bg-primary text-white",
                h5 { class: "card-title mb-0",
                    i { class: "bi {icon} me-2" }
                    "{title}"
                }
            }
            div { class: "card-body",
                div {
                    class: "alert {alert_class} d-flex align-items-center",
                    role: "alert",
                    i { class: "bi {icon} {icon_class} me-2" }
                    div {
                        "Verbunden als: "
                        strong { "{user_info.mitgliedsnr}" }
                        pre { "Identity: {user_info.decode_id_token():#?}" }
                        div { "Identity: {identity:?}" }
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
                                span { class: "badge bg-primary", "{accounts_count}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AccountsSection(accounts: Signal<Vec<Account>>) -> Element {
    let spacetime_db: use_spacetime_db::SpacetimeDb = use_context();

    rsx! {
        div { class: "card shadow-sm",
            div { class: "card-header bg-primary text-white",
                h5 { class: "card-title mb-0",
                    i { class: "bi bi-people-fill me-2" }
                    "SoLaWi Mitglieder"
                }
            }
            div { class: "card-body",
                if accounts.read().is_empty() {
                    div {
                        class: "alert alert-info d-flex align-items-center",
                        role: "alert",
                        i { class: "bi bi-info-circle me-2" }
                        "Keine Accounts gefunden. Verbindung zur Datenbank prüfen."
                    }
                } else {
                    div { class: "row",
                        for user in accounts.read().iter() {
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
                                                let spacetime_db = spacetime_db.clone();
                                                let user_name = user.name.clone();
                                                let user_email = user.email.clone();
                                                move |_| {
                                                    info!("Adding message category for user: {} ({})", user_name, user_email);
                                                    if let Some(db) = spacetime_db.connection.as_ref() {
                                                        match db
                                                            .reducers
                                                            .add_message_category(
                                                                user_name.clone(),
                                                                user_email.clone(),
                                                                "Standard Kategorie".to_string(),
                                                            )
                                                        {
                                                            Ok(_) => {
                                                                info!("Successfully added message category for {}", user_name);
                                                            }
                                                            Err(e) => {
                                                                error!(
                                                                    "Failed to add message category for {}: {:?}", user_name, e
                                                                );
                                                            }
                                                        }
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
