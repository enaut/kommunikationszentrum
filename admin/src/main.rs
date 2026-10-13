mod components;
mod config;
mod i18n;
pub mod mime_parser;
mod module_bindings;
mod oauth;
mod pages;
mod router;

use ::dioxus::{logger::tracing::info, prelude::*};
use config::AdminConfig;
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;
use module_bindings::dioxus::{
    ConnectionState, use_connection_state, use_spacetimedb_context_provider,
};
use oauth::{AuthState, UserInfo, use_oauth};
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
    let _i18n = i18n::use_init_app_i18n();
    let config = use_signal(AdminConfig::load);
    let (auth_state, login, logout) = use_oauth(config.read().oauth.clone());

    let mut unsubscribe_token = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            let params = oauth::auth_flow::parse_url_params();
            if let Some(token) = params
                .get("unsubscribe_token")
                .or_else(|| params.get("unsubscribe"))
            {
                Some(token.clone())
            } else if params.get("action").map(|s| s.as_str()) == Some("unsubscribe") {
                params.get("token").cloned()
            } else {
                None
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            None::<String>
        }
    });

    let mut verification_token = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            let params = oauth::auth_flow::parse_url_params();
            if params.contains_key("unsubscribe_token")
                || params.contains_key("unsubscribe")
                || params.get("action").map(|s| s.as_str()) == Some("unsubscribe")
            {
                None
            } else {
                params.get("token").cloned()
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            None::<String>
        }
    });

    // Theme signal for ThemeProvider + ThemeToggle
    let theme = use_signal(|| i18n::detect_initial_theme());

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        ThemeProvider { theme }
        BootstrapHead {}
        BootstrapThemeProvider { theme: solawi_theme() }
        if let Some(token) = unsubscribe_token.cloned() {
            {
                let (is_authenticated, id_token) = match &*auth_state.read() {
                    AuthState::Authenticated(u) => (true, u.spacetimedb_token()),
                    _ => (false, None),
                };
                rsx! {
                    pages::unsubscribe::UnsubscribePage {
                        token,
                        on_login: login,
                        on_continue: if is_authenticated {
                            Some(Callback::new(move |_| unsubscribe_token.set(None)))
                        } else {
                            None
                        },
                        id_token,
                    }
                }
            }
        } else if let Some(token) = verification_token.cloned() {
            {
                let (is_authenticated, id_token) = match &*auth_state.read() {
                    AuthState::Authenticated(u) => (true, u.spacetimedb_token()),
                    _ => (false, None),
                };
                rsx! {
                    pages::verify_email::VerifyEmailPage {
                        token,
                        on_login: login,
                        on_continue: if is_authenticated {
                            Some(Callback::new(move |_| verification_token.set(None)))
                        } else {
                            None
                        },
                        id_token,
                    }
                }
            }
        } else {
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
                    h4 { class: "mt-2 mb-0", {tid!("app-login-title")} }
                    p { class: "text-muted small", {tid!("app-login-subtitle")} }
                }
                Button {
                    color: Color::Primary,
                    class: "w-100",
                    onclick: move |_| on_login.call(()),
                    Icon { name: "box-arrow-in-right", class: "me-2" }
                    {tid!("app-login-button")}
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
                Spinner { color: Color::Primary, class: "mb-3", {tid!("app-loading")} }
                p { class: "text-muted", {tid!("app-authenticating")} }
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
                h5 { class: "mt-3 text-danger", {tid!("app-authentication-error")} }
                p { class: "text-muted small mb-4", "{error}" }
                Button {
                    color: Color::Primary,
                    onclick: move |_| on_retry.call(()),
                    Icon { name: "arrow-clockwise", class: "me-2" }
                    {tid!("app-retry")}
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

    info!("Authenticated as: {}", user_info.subject_id);

    let _ctx = use_spacetimedb_context_provider(&uri, &module_name, user_info.spacetimedb_token());

    let state = use_connection_state();
    let register_self_async = module_bindings::dioxus::use_reducer_register_self_async();
    let mut registered = use_signal(|| false);
    let mut is_registering = use_signal(|| false);
    let mut registration_error = use_signal(|| None::<String>);

    let user_info_for_effect = user_info.clone();
    use_effect(move || {
        let conn_state = state();
        if matches!(
            conn_state,
            ConnectionState::Connecting | ConnectionState::Reconnecting { .. }
        ) {
            if registration_error.read().is_some() {
                registration_error.set(None);
            }
            return;
        }

        if matches!(conn_state, ConnectionState::Connected(_, _))
            && !*registered.read()
            && !*is_registering.read()
            && registration_error.read().is_none()
        {
            is_registering.set(true);
            let name = user_info_for_effect
                .name
                .clone()
                .or_else(|| {
                    match (
                        &user_info_for_effect.given_name,
                        &user_info_for_effect.family_name,
                    ) {
                        (Some(g), Some(f)) => Some(format!("{} {}", g, f)),
                        (Some(g), None) => Some(g.clone()),
                        (None, Some(f)) => Some(f.clone()),
                        (None, None) => None,
                    }
                })
                .unwrap_or_else(|| user_info_for_effect.username.clone());
            let subject = user_info_for_effect.subject_id.clone();
            let register_fn = register_self_async.clone();

            spawn(async move {
                match register_fn(subject, name).await {
                    Ok(()) => {
                        ::dioxus::logger::tracing::info!(
                            "Successfully registered self in SpacetimeDB"
                        );
                        registered.set(true);
                        is_registering.set(false);
                        registration_error.set(None);
                    }
                    Err(e) => {
                        ::dioxus::logger::tracing::warn!("Failed to call register_self: {:?}", e);
                        registration_error.set(Some(format!("{:?}", e)));
                        is_registering.set(false);
                    }
                }
            });
        }
    });

    let active_view = use_signal(|| ActiveView::MySubscriptions);

    module_bindings::dioxus::use_subscription(&["SELECT * FROM visible_account_configs"]);
    let account_configs = module_bindings::dioxus::use_table_visible_account_configs();
    let i18n = i18n::use_i18n();
    let mut theme_for_sync = theme;
    let update_config_sync = module_bindings::dioxus::use_reducer_update_account_config();

    let update_sync_1 = update_config_sync.clone();
    let update_sync_2 = update_config_sync.clone();

    use_effect(move || {
        let configs = account_configs();
        if let Some(cfg) = configs.first() {
            // Sync language from SpacetimeDB to i18n and localStorage
            if let Some(lang) = &cfg.language {
                let current_lang = i18n.language();
                let current_lang_str = current_lang.language.as_str();
                if current_lang_str != lang {
                    if lang == "de" {
                        i18n::switch_language(i18n, unic_langid::langid!("de"));
                    } else if lang == "en" {
                        i18n::switch_language(i18n, unic_langid::langid!("en"));
                    }
                }
            } else {
                let current_lang = i18n.language();
                let current_lang_str = current_lang.language.as_str().to_string();
                let _ = update_sync_1(
                    None,
                    None,
                    None,
                    false,
                    None,
                    None,
                    None,
                    false,
                    None,
                    false,
                    Some(current_lang_str),
                    None,
                    None,
                    false,
                );
            }

            // Sync theme from SpacetimeDB to theme signal and localStorage, or seed remote if None
            let current_local_theme = *theme_for_sync.read();
            if let Some(t_str) = &cfg.theme {
                let target_theme = if t_str.eq_ignore_ascii_case("dark") {
                    Theme::Dark
                } else {
                    Theme::Light
                };
                if current_local_theme != target_theme {
                    theme_for_sync.set(target_theme);
                    i18n::save_theme(t_str);
                }
            } else {
                let current_theme_str = match current_local_theme {
                    Theme::Dark => "dark".to_string(),
                    _ => "light".to_string(),
                };
                let _ = update_sync_1(
                    None,
                    None,
                    None,
                    false,
                    None,
                    None,
                    None,
                    false,
                    None,
                    false,
                    None,
                    Some(current_theme_str),
                    None,
                    false,
                );
            }
        }
    });

    // Also watch local theme signal changes to persist back to SpacetimeDB and localStorage
    use_effect(move || {
        let current_theme = *theme.read();
        let current_theme_str = match current_theme {
            Theme::Dark => "dark",
            _ => "light",
        };
        let configs = account_configs();
        if let Some(cfg) = configs.first() {
            if cfg.theme.as_deref() != Some(current_theme_str) {
                i18n::save_theme(current_theme_str);
                let _ = update_sync_2(
                    None,
                    None,
                    None,
                    false,
                    None,
                    None,
                    None,
                    false,
                    None,
                    false,
                    None,
                    Some(current_theme_str.to_string()),
                    None,
                    false,
                );
            }
        }
    });

    rsx! {
        components::navbar::Navbar {
            user_info: user_info.clone(),
            active_view,
            on_logout,
            theme: theme.clone(),
        }
        if let Some(err) = registration_error.read().as_ref() {
            Container { class: "mt-3",
                Alert { color: Color::Danger, class: "d-flex align-items-center justify-content-between",
                    div { class: "d-flex align-items-center",
                        Icon { name: "exclamation-triangle", class: "me-2" }
                        span { {tid!("app-registration-failed", error: err)} }
                    }
                    Button {
                        color: Color::Danger,
                        size: Size::Sm,
                        class: "ms-3",
                        onclick: move |_| registration_error.set(None),
                        {tid!("app-retry")}
                    }
                }
            }
        }
        {
            match state() {
                ConnectionState::Connected(id, _) => {
                    info!("Connected to SpacetimeDB as: {}", id);
                    match active_view() {
                        ActiveView::MySubscriptions => rsx! {
                            pages::subscriptions::SubscriptionsPage { user_info: user_info.clone(), active_view }
                        },
                        ActiveView::Messages => rsx! {
                            pages::messages::MessagesPage { user_info: user_info.clone() }
                        },
                        ActiveView::Topics => rsx! {
                            pages::topic::TopicsPage {}
                        },
                        ActiveView::Members => rsx! {
                            pages::members::MembersPage { user_info: user_info.clone() }
                        },
                        ActiveView::ManagementConfiguration => rsx! {
                            pages::management::configuration::ManagementConfigurationPage {}
                        },
                        ActiveView::ManagementStatus => rsx! {
                            pages::management::status::ManagementStatusPage { user_info: user_info.clone() }
                        },
                        ActiveView::Profile => rsx! {
                            pages::profile::ProfilePage { user_info: user_info.clone(), theme }
                        },
                    }
                }
                ConnectionState::Connecting | ConnectionState::Reconnecting { .. } => {
                    rsx! {
                        div { class: "d-flex justify-content-center align-items-center mt-5",
                            div { class: "text-center",
                                Spinner { color: Color::Primary, class: "mb-3", {tid!("app-loading")} }
                                p { class: "text-muted", {tid!("app-connection-loading")} }
                            }
                        }
                    }
                }
                _ => rsx! {
                    Container { class: "mt-5",
                        Alert { color: Color::Danger, class: "d-flex align-items-center",
                            Icon { name: "exclamation-circle", class: "me-2" }
                            {tid!("app-connection-lost")}
                        }
                    }
                },
            }
        }
    }
}
