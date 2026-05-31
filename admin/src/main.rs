mod components;
mod config;
mod module_bindings;
mod oauth;
mod pages;
mod router;

use ::dioxus::{logger::tracing::info, prelude::*};
use config::AdminConfig;
use module_bindings::dioxus::{
    use_connection_state, use_spacetimedb_context_provider, use_subscription, ConnectionState,
};
use oauth::{use_oauth, AuthState, UserInfo};
use router::ActiveView;

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

// ---------------------------------------------------------------------------
// Auth pages — shown before / instead of the main app
// ---------------------------------------------------------------------------

#[component]
fn LoginPage(on_login: Callback<()>) -> Element {
    rsx! {
        div { class: "d-flex justify-content-center align-items-center vh-100 bg-light",
            div { class: "card shadow p-4", style: "min-width: 320px;",
                div { class: "text-center mb-4",
                    i { class: "bi bi-envelope-fill text-primary", style: "font-size: 3rem;" }
                    h4 { class: "mt-2 mb-0", "Kommunikationszentrum" }
                    p { class: "text-muted small", "SoLaWi Nachrichtenkategorien" }
                }
                button {
                    class: "btn btn-primary w-100",
                    onclick: move |_| on_login.call(()),
                    i { class: "bi bi-box-arrow-in-right me-2" }
                    "Mit SoLaWi-Account anmelden"
                }
            }
        }
    }
}

#[component]
fn AuthenticatingPage() -> Element {
    rsx! {
        div { class: "d-flex justify-content-center align-items-center vh-100",
            div { class: "text-center",
                div { class: "spinner-border text-primary mb-3", role: "status" }
                p { class: "text-muted", "Anmeldung wird verarbeitet…" }
            }
        }
    }
}

#[component]
fn ErrorPage(error: String, on_retry: Callback<()>) -> Element {
    rsx! {
        div { class: "d-flex justify-content-center align-items-center vh-100 bg-light",
            div { class: "card shadow p-4 text-center", style: "min-width: 320px;",
                i {
                    class: "bi bi-exclamation-triangle-fill text-danger",
                    style: "font-size: 2.5rem;",
                }
                h5 { class: "mt-3 text-danger", "Authentifizierungsfehler" }
                p { class: "text-muted small mb-4", "{error}" }
                button {
                    class: "btn btn-primary",
                    onclick: move |_| on_retry.call(()),
                    i { class: "bi bi-arrow-clockwise me-2" }
                    "Erneut versuchen"
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main authenticated shell
// ---------------------------------------------------------------------------

#[component]
fn AuthenticatedApp(user_info: UserInfo, on_logout: EventHandler<()>) -> Element {
    let config = use_signal(AdminConfig::load);
    let uri = config.read().spacetimedb_uri.clone();
    let module_name = config.read().spacetimedb_module_name.clone();

    info!("Authenticated as: {}", user_info.mitgliedsnr);

    let _ctx = use_spacetimedb_context_provider(&uri, &module_name, user_info.id_token.clone());

    use_subscription(&[
        "SELECT * FROM visible_accounts",
        "SELECT * FROM admin_identities",
        "SELECT * FROM message_categories",
        "SELECT * FROM visible_subscriptions",
    ]);

    let state = use_connection_state();
    let active_view = use_signal(|| ActiveView::MySubscriptions);

    rsx! {
        components::navbar::Navbar {
            user_info: user_info.clone(),
            active_view,
            on_logout,
        }
        {
            match state() {
                ConnectionState::Connected(_, _) => match active_view() {
                    ActiveView::MySubscriptions => rsx! {
                        pages::subscriptions::SubscriptionsPage {
                            user_info: user_info.clone(),
                        }
                    },
                    ActiveView::Categories => rsx! {
                        pages::categories::CategoriesPage {}
                    },
                    ActiveView::Members => rsx! {
                        pages::members::MembersPage {}
                    },
                    ActiveView::Debug => rsx! {
                        pages::debug::DebugPage { user_info: user_info.clone() }
                    },
                },
                ConnectionState::Connecting | ConnectionState::Reconnecting { .. } => rsx! {
                    div { class: "d-flex justify-content-center align-items-center mt-5",
                        div { class: "text-center",
                            div { class: "spinner-border text-primary mb-3", role: "status" }
                            p { class: "text-muted",
                                "Verbindung zu SpacetimeDB wird hergestellt…"
                            }
                        }
                    }
                },
                _ => rsx! {
                    div { class: "container mt-5",
                        div { class: "alert alert-danger d-flex align-items-center",
                            i { class: "bi bi-exclamation-circle me-2" }
                            "Verbindung zu SpacetimeDB getrennt oder fehlgeschlagen."
                        }
                    }
                },
            }
        }
    }
}
