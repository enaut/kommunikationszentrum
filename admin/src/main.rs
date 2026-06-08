mod components;
mod config;
mod module_bindings;
mod oauth;
mod pages;
mod router;

use ::dioxus::{logger::tracing::info, prelude::*};
use config::AdminConfig;
use dioxus_bootstrap_css::prelude::*;
use module_bindings::dioxus::{
    use_connection_state, use_spacetimedb_context_provider, use_subscription, ConnectionState,
};
use oauth::{use_oauth, AuthState, UserInfo};
use router::ActiveView;

const FAVICON: Asset = asset!("/assets/favicon.ico");

fn main() {
    ::dioxus::launch(App);
}

fn solawi_theme() -> BootstrapTheme {
    BootstrapTheme {
        colors: ThemeColors {
            primary: Some(SemanticColorScale::new("#165317")),
            secondary: Some(SemanticColorScale::new("#5c6b5d")),
            success: Some(SemanticColorScale::new("#2f9e44")),
            info: Some(SemanticColorScale::new("#0b7285")),
            warning: Some(SemanticColorScale::new("#e67700")),
            danger: Some(SemanticColorScale::new("#c92a2a")),
            light: Some(SemanticColorScale::new("#e9f2ea")),
            dark: Some(SemanticColorScale::new("#092817")),
        },
        surfaces: SurfaceColors {
            body_bg: Some("#f7fbf7".into()),
            body_color: Some("#165317".into()),
            secondary_bg: Some("#edf3ed".into()),
            secondary_color: Some("#314033".into()),
            tertiary_bg: None,
            tertiary_color: None,
            border_color: Some("#d4dfd4".into()),
            link_color: None,
            link_hover_color: None,
        },
        dark: Some(ThemeModeTokens {
            colors: ThemeColors {
                primary: Some(SemanticColorScale::new("#1D480D")),
                secondary: Some(SemanticColorScale::new("#7b8b7d")),
                success: Some(SemanticColorScale::new("#004E00")),
                info: Some(SemanticColorScale::new("#15b")),
                warning: Some(SemanticColorScale::new("#940")),
                danger: Some(SemanticColorScale::new("#922")),
                light: Some(SemanticColorScale::new("#49624a")),
                dark: Some(SemanticColorScale::new("#081e12")),
            },
            surfaces: SurfaceColors {
                body_bg: Some("#0b120c".into()),
                body_color: Some("#e6efe7".into()),
                secondary_bg: Some("#162019".into()),
                secondary_color: Some("#c7d3c8".into()),
                tertiary_bg: None,
                tertiary_color: None,
                border_color: Some("#2a382d".into()),
                link_color: Some("#74c97d".into()),
                link_hover_color: Some("#95d89c".into()),
            },
        }),
        ..BootstrapTheme::default()
    }
}

#[component]
fn App() -> Element {
    let config = use_signal(AdminConfig::load);
    let (auth_state, login, logout) = use_oauth(config.read().oauth.clone());

    // Theme signal for ThemeProvider + ThemeToggle
    let theme = use_signal(|| Theme::Light);

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        ThemeProvider { theme }
        BootstrapHead {}
        BootstrapThemeProvider { theme: solawi_theme() }
        match &*auth_state.read() {
            AuthState::Unauthenticated => rsx! {
                LoginPage { on_login: login }
            },
            AuthState::Authenticating => rsx! {
                AuthenticatingPage {}
            },
            AuthState::Authenticated(user_info) => rsx! {
                AuthenticatedApp { user_info: user_info.clone(), on_logout: logout, theme }
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
            Card { class: "shadow p-4", style: "min-width: 320px;",
                div { class: "text-center mb-4",
                    Icon { name: "envelope-fill", class: "text-primary" }
                    h4 { class: "mt-2 mb-0", "Kommunikationszentrum" }
                    p { class: "text-muted small", "SoLaWi Nachrichten verwaltung" }
                }
                Button {
                    color: Color::Primary,
                    class: "w-100",
                    onclick: move |_| on_login.call(()),
                    Icon { name: "box-arrow-in-right", class: "me-2" }
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
                Spinner { color: Color::Primary, class: "mb-3", "Laden…" }
                p { class: "text-muted", "Anmeldung wird verarbeitet…" }
            }
        }
    }
}

#[component]
fn ErrorPage(error: String, on_retry: Callback<()>) -> Element {
    rsx! {
        div { class: "d-flex justify-content-center align-items-center vh-100 bg-light",
            Card { class: "shadow p-4 text-center", style: "min-width: 320px;",
                Icon { name: "exclamation-triangle-fill", class: "text-danger" }
                h5 { class: "mt-3 text-danger", "Authentifizierungsfehler" }
                p { class: "text-muted small mb-4", "{error}" }
                Button {
                    color: Color::Primary,
                    onclick: move |_| on_retry.call(()),
                    Icon { name: "arrow-clockwise", class: "me-2" }
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
fn AuthenticatedApp(
    user_info: UserInfo,
    on_logout: EventHandler<()>,
    theme: Signal<Theme>,
) -> Element {
    let config = use_signal(AdminConfig::load);
    let uri = config.read().spacetimedb_uri.clone();
    let module_name = config.read().spacetimedb_module_name.clone();

    info!("Authenticated as: {}", user_info.mitgliedsnr);

    let _ctx = use_spacetimedb_context_provider(&uri, &module_name, user_info.id_token.clone());

    use_subscription(&[
        "SELECT * FROM visible_accounts",
        "SELECT * FROM visible_admin_identities",
        "SELECT * FROM message_categories",
        "SELECT * FROM visible_subscriptions",
        "SELECT * FROM visible_messages",
        "SELECT * FROM visible_webhook_tokens",
    ]);

    let state = use_connection_state();
    let active_view = use_signal(|| ActiveView::MySubscriptions);

    rsx! {
        components::navbar::Navbar {
            user_info: user_info.clone(),
            active_view,
            on_logout,
            theme: theme.clone(),
        }
        {
            match state() {
                ConnectionState::Connected(_, _) => {
                    match active_view() {
                        ActiveView::MySubscriptions => rsx! {
                            pages::subscriptions::SubscriptionsPage { user_info: user_info.clone() }
                        },
                        ActiveView::Messages => rsx! {
                            pages::messages::MessagesPage {}
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
                    }
                }
                ConnectionState::Connecting | ConnectionState::Reconnecting { .. } => {
                    rsx! {
                        div { class: "d-flex justify-content-center align-items-center mt-5",
                            div { class: "text-center",
                                Spinner { color: Color::Primary, class: "mb-3", "Laden…" }
                                p { class: "text-muted", "Verbindung zu SpacetimeDB wird hergestellt…" }
                            }
                        }
                    }
                }
                _ => rsx! {
                    Container { class: "mt-5",
                        Alert { color: Color::Danger, class: "d-flex align-items-center",
                            Icon { name: "exclamation-circle", class: "me-2" }
                            "Verbindung zu SpacetimeDB getrennt oder fehlgeschlagen."
                        }
                    }
                },
            }
        }
    }
}
