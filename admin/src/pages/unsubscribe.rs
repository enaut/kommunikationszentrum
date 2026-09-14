use dioxus::prelude::*;
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;
use tracing::info;

use crate::config::AdminConfig;
use crate::module_bindings::dioxus::{
    use_connection_state, use_reducer_user_unsubscribe_by_token_then,
    use_spacetimedb_context_provider, ConnectionState,
};
use crate::oauth::auth_flow::clear_url;

#[component]
pub fn UnsubscribePage(
    token: String,
    on_login: Callback<()>,
    on_continue: Option<Callback<()>>,
    id_token: Option<String>,
) -> Element {
    let config = use_signal(AdminConfig::load);
    let uri = config.read().spacetimedb_uri.clone();
    let module_name = config.read().spacetimedb_module_name.clone();

    let _ctx = use_spacetimedb_context_provider(&uri, &module_name, id_token);

    let state = use_connection_state();
    let (unsubscribe, unsubscribe_result) = use_reducer_user_unsubscribe_by_token_then();
    let mut submitted = use_signal(|| false);

    let token_clone = token.clone();
    use_effect(move || {
        if matches!(state(), ConnectionState::Connected(_, _)) && !submitted() {
            submitted.set(true);
            info!("SpacetimeDB connected, submitting unsubscribe token...");
            unsubscribe(token_clone.clone());
            clear_url();
        }
    });

    rsx! {
        div { class: "d-flex justify-content-center align-items-center vh-100 bg-light",
            Card { class: "shadow p-4 text-center", style: "min-width: 340px; max-width: 480px;",
                match &*unsubscribe_result.read() {
                    None => rsx! {
                        if matches!(state(), ConnectionState::Error) {
                            div { class: "text-danger mb-3",
                                Icon { name: "exclamation-triangle-fill", class: "text-danger" }
                            }
                            h5 { class: "mt-2 text-danger", {tid!("app-connection-lost")} }
                            p { class: "text-muted small mb-4", "Could not connect to SpacetimeDB." }
                            if let Some(on_continue) = on_continue {
                                Button {
                                    color: Color::Primary,
                                    class: "w-100",
                                    onclick: move |_| on_continue.call(()),
                                    Icon { name: "arrow-right", class: "me-2" }
                                    {tid!("unsubscribe-continue")}
                                }
                            } else {
                                Button {
                                    color: Color::Primary,
                                    class: "w-100",
                                    onclick: move |_| on_login.call(()),
                                    Icon { name: "box-arrow-in-right", class: "me-2" }
                                    {tid!("app-login-button")}
                                }
                            }
                        } else {
                            Spinner { color: Color::Primary, class: "mb-3", {tid!("app-loading")} }
                            h4 { class: "mt-2 mb-1", {tid!("unsubscribe-processing")} }
                            p { class: "text-muted small mb-0", {tid!("unsubscribe-please-wait")} }
                        }
                    },
                    Some(Ok(())) => rsx! {
                        div { class: "text-success mb-3",
                            Icon { name: "check-circle-fill", class: "text-success" }
                        }
                        h4 { class: "text-success mb-2", {tid!("unsubscribe-success-title")} }
                        p { class: "text-muted mb-4", {tid!("unsubscribe-success-desc")} }
                        if let Some(on_continue) = on_continue {
                            Button {
                                color: Color::Primary,
                                class: "w-100",
                                onclick: move |_| on_continue.call(()),
                                Icon { name: "arrow-right", class: "me-2" }
                                {tid!("unsubscribe-continue")}
                            }
                        } else {
                            Button {
                                color: Color::Primary,
                                class: "w-100",
                                onclick: move |_| on_login.call(()),
                                Icon { name: "box-arrow-in-right", class: "me-2" }
                                {tid!("app-login-button")}
                            }
                        }
                    },
                    Some(Err(err)) => rsx! {
                        div { class: "text-danger mb-3",
                            Icon { name: "exclamation-circle-fill", class: "text-danger" }
                        }
                        h4 { class: "text-danger mb-2", {tid!("unsubscribe-failed-title")} }
                        p { class: "text-muted mb-4",
                            {tid!("unsubscribe-failed-desc")}
                            br {}
                            small { class: "text-muted", "({err})" }
                        }
                        if let Some(on_continue) = on_continue {
                            Button {
                                color: Color::Primary,
                                class: "w-100",
                                onclick: move |_| on_continue.call(()),
                                Icon { name: "arrow-right", class: "me-2" }
                                {tid!("unsubscribe-continue")}
                            }
                        } else {
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
        }
    }
}
