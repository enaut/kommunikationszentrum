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

fn main() {
    ::dioxus::launch(App);
}

fn configured_theme() -> BootstrapTheme {
    let to_scale = |s: Option<&'static str>| s.map(SemanticColorScale::new);
    let to_string = |s: Option<&'static str>| s.map(|v| v.to_string());

    let light_colors = config::generated::GENERATED_THEME.colors;
    let light_surfaces = config::generated::GENERATED_THEME.surfaces;

    let dark_tokens = config::generated::GENERATED_THEME
        .dark
        .map(|dark| ThemeModeTokens {
            colors: ThemeColors {
                primary: to_scale(dark.colors.primary),
                secondary: to_scale(dark.colors.secondary),
                success: to_scale(dark.colors.success),
                info: to_scale(dark.colors.info),
                warning: to_scale(dark.colors.warning),
                danger: to_scale(dark.colors.danger),
                light: to_scale(dark.colors.light),
                dark: to_scale(dark.colors.dark),
            },
            surfaces: SurfaceColors {
                body_bg: to_string(dark.surfaces.body_bg),
                body_color: to_string(dark.surfaces.body_color),
                secondary_bg: to_string(dark.surfaces.secondary_bg),
                secondary_color: to_string(dark.surfaces.secondary_color),
                tertiary_bg: None,
                tertiary_color: None,
                border_color: to_string(dark.surfaces.border_color),
                link_color: to_string(dark.surfaces.link_color),
                link_hover_color: to_string(dark.surfaces.link_hover_color),
            },
        });

    BootstrapTheme {
        colors: ThemeColors {
            primary: to_scale(light_colors.primary),
            secondary: to_scale(light_colors.secondary),
            success: to_scale(light_colors.success),
            info: to_scale(light_colors.info),
            warning: to_scale(light_colors.warning),
            danger: to_scale(light_colors.danger),
            light: to_scale(light_colors.light),
            dark: to_scale(light_colors.dark),
        },
        surfaces: SurfaceColors {
            body_bg: to_string(light_surfaces.body_bg),
            body_color: to_string(light_surfaces.body_color),
            secondary_bg: to_string(light_surfaces.secondary_bg),
            secondary_color: to_string(light_surfaces.secondary_color),
            tertiary_bg: None,
            tertiary_color: None,
            border_color: to_string(light_surfaces.border_color),
            link_color: to_string(light_surfaces.link_color),
            link_hover_color: to_string(light_surfaces.link_hover_color),
        },
        dark: dark_tokens,
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
        document::Title { "{config::generated::GENERATED_BRAND.page_title}" }
        document::Link { rel: "icon", href: "{config::generated::GENERATED_BRAND.favicon}" }
        ThemeProvider { theme }
        BootstrapHead {}
        BootstrapThemeProvider { theme: configured_theme() }
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
                AuthState::Authenticated(user_info) => {
                    if let Some(token) = user_info.spacetimedb_token() {
                        rsx! {
                            AuthenticatedApp { user_info: user_info.clone(), token, on_logout: logout, theme }
                        }
                    } else {
                        rsx! {
                            ErrorPage {
                                error: tid!("app-session-expired"),
                                on_retry: login,
                            }
                        }
                    }
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
                    Icon { name: config::generated::GENERATED_BRAND.icon, class: "text-primary display-1" }
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
    token: String,
    on_logout: EventHandler<()>,
    theme: Signal<Theme>,
) -> Element {
    let mut current_token = use_signal(|| token.clone());
    if *current_token.peek() != token {
        current_token.set(token.clone());
    }

    let user_info_for_token_expiry = user_info.clone();
    use_effect(move || {
        let active = current_token.read().clone();
        if let Ok(jwt) = crate::oauth::jwt_utils::decode_jwt(&active) {
            if let Some(exp) = crate::oauth::jwt_utils::get_jwt_exp(&jwt.claims) {
                let now = crate::oauth::jwt_utils::current_unix_timestamp();
                // Check 30s before expiration (with minimum 1s)
                let wait_secs = exp.saturating_sub(now).saturating_sub(30).max(1);
                let u_info = user_info_for_token_expiry.clone();
                let mut current_token = current_token;
                spawn(async move {
                    gloo_timers::future::TimeoutFuture::new(
                        (wait_secs * 1000).min(u32::MAX as u64) as u32,
                    )
                    .await;
                    if let Some(next_token) = u_info.spacetimedb_token() {
                        if *current_token.peek() == active && next_token != active {
                            info!("Proactively switching to unexpired SpacetimeDB token");
                            current_token.set(next_token);
                        }
                    }
                });
            }
        }
    });

    let config = use_signal(AdminConfig::load);
    let uri = config.read().spacetimedb_uri.clone();
    let module_name = config.read().spacetimedb_module_name.clone();

    info!("Authenticated as: {}", user_info.subject_id);

    let _ctx =
        use_spacetimedb_context_provider(&uri, &module_name, Some(current_token.read().clone()));

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
    let suggest_theme_and_lang = module_bindings::dioxus::use_reducer_suggest_theme_and_lang();
    let mut has_suggested = use_signal(|| false);

    use_effect(move || {
        let configs = account_configs();
        if let Some(cfg) = configs.first() {
            // Seed SpacetimeDB if remote preferences are None
            if (cfg.theme.is_none() || cfg.language.is_none()) && !*has_suggested.peek() {
                has_suggested.set(true);
                let initial_theme_pref = i18n::theme_to_preference(*theme_for_sync.peek());
                let initial_lang_pref = i18n::langid_to_preference(&i18n.language());
                let theme_arg = if cfg.theme.is_none() {
                    Some(initial_theme_pref)
                } else {
                    None
                };
                let lang_arg = if cfg.language.is_none() {
                    Some(initial_lang_pref)
                } else {
                    None
                };
                let _ = suggest_theme_and_lang(theme_arg, lang_arg);
            }

            // Sync theme from SpacetimeDB to theme signal and localStorage
            if let Some(theme_pref) = cfg.theme {
                let target_theme = i18n::preference_to_theme(theme_pref);
                if *theme_for_sync.peek() != target_theme {
                    theme_for_sync.set(target_theme);
                    let theme_str = match target_theme {
                        Theme::Dark => "dark",
                        Theme::Light => "light",
                    };
                    i18n::save_theme(theme_str);
                }
            }

            // Sync language from SpacetimeDB to i18n and localStorage
            if let Some(lang_pref) = cfg.language {
                let target_lang = i18n::preference_to_langid(lang_pref);
                let current_lang = i18n.language();
                if current_lang.language != target_lang.language {
                    i18n::switch_language(i18n, target_lang);
                }
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
