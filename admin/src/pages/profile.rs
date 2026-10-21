use dioxus::prelude::*;
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;
use wasm_bindgen_futures::{JsFuture, spawn_local};

use crate::i18n::use_i18n;
use crate::module_bindings::dioxus::{
    ConnectionState, use_connection_state, use_reducer_remove_account_email,
    use_reducer_set_theme_and_lang, use_reducer_user_request_email_verification, use_subscription,
    use_table_context_accounts, use_table_visible_account_emails, use_table_visible_accounts,
};
use crate::module_bindings::{EmailSource, LanguagePreference, ThemePreference};
use crate::oauth::UserInfo;
use crate::router::use_is_admin;

#[component]
pub fn ProfilePage(user_info: UserInfo, theme: Signal<Theme>) -> Element {
    use_subscription(&[
        "SELECT * FROM visible_accounts",
        "SELECT * FROM context_accounts",
        "SELECT * FROM visible_account_emails",
        "SELECT * FROM visible_account_configs",
        "SELECT * FROM visible_admin_identities",
    ]);

    let accounts = use_table_visible_accounts();
    let context_accounts = use_table_context_accounts();
    let account_emails = use_table_visible_account_emails();
    let is_admin = use_is_admin();
    let conn_state = use_connection_state();

    let add_email = use_reducer_user_request_email_verification();
    let remove_email = use_reducer_remove_account_email();
    let set_theme_and_lang = use_reducer_set_theme_and_lang();

    let mut show_add_email = use_signal(|| false);
    let mut add_email_input = use_signal(|| String::new());
    let mut copied_identity = use_signal(|| false);
    let resent_email_id: Signal<Option<u64>> = use_signal(|| None);

    let i18n = use_i18n();
    let current_lang = i18n.language();
    let current_lang_code = current_lang.language.as_str().to_string();

    let my_account = context_accounts()
        .into_iter()
        .find(|a| a.external_id == user_info.subject_id)
        .or_else(|| {
            accounts()
                .into_iter()
                .find(|a| a.external_id == user_info.subject_id)
        });
    let account_id = my_account.as_ref().map(|a| a.id).unwrap_or(0);
    let my_primary_email_id = my_account.as_ref().map(|a| a.primary_email_id).unwrap_or(0);
    let my_emails: Vec<_> = account_emails()
        .into_iter()
        .filter(|e| e.account_id == account_id)
        .collect();

    let display_name = user_info
        .name
        .clone()
        .or_else(|| match (&user_info.given_name, &user_info.family_name) {
            (Some(g), Some(f)) => Some(format!("{} {}", g, f)),
            (Some(g), None) => Some(g.clone()),
            (None, Some(f)) => Some(f.clone()),
            (None, None) => None,
        })
        .or_else(|| my_account.as_ref().map(|a| a.name.clone()))
        .unwrap_or_else(|| user_info.username.clone());

    let primary_email_str = my_emails
        .iter()
        .find(|e| e.id == my_primary_email_id)
        .map(|e| e.email.clone())
        .or_else(|| user_info.email.clone())
        .unwrap_or_else(|| "-".to_string());

    let identity_hex = match conn_state() {
        ConnectionState::Connected(id, _) => id.to_hex().to_string(),
        _ => my_account
            .as_ref()
            .map(|a| a.identity.to_hex().to_string())
            .unwrap_or_default(),
    };

    let is_dark = *theme.read() == Theme::Dark;

    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "person-circle", class: "me-2" }
                        {tid!("profile-page-title")}
                    }
                    p { class: "text-muted mt-1",
                        {tid!("profile-page-description")}
                    }
                }
            }

            Row {
                // Left column: Account Overview & Settings
                Col { md: ColumnSize::Span(6), class: "mb-4",
                    Card { class: "shadow-sm mb-4",
                        header: rsx! {
                            h5 { class: "mb-0",
                                Icon { name: "person-badge", class: "me-2" }
                                {tid!("profile-account-overview-title")}
                            }
                        },
                        body: rsx! {
                            div { class: "row g-3",
                                div { class: "col-sm-6",
                                    small { class: "text-muted d-block", {tid!("profile-account-name")} }
                                    strong { "{display_name}" }
                                }
                                div { class: "col-sm-6",
                                    small { class: "text-muted d-block", {tid!("profile-account-username")} }
                                    span { class: "font-monospace", "{user_info.username}" }
                                }
                                div { class: "col-sm-6",
                                    small { class: "text-muted d-block", {tid!("profile-account-status")} }
                                    Badge { color: Color::Success, {tid!("profile-account-status-active")} }
                                }
                                div { class: "col-sm-6",
                                    small { class: "text-muted d-block", {tid!("profile-account-role")} }
                                    if is_admin {
                                        Badge { color: Color::Warning, {tid!("profile-role-admin")} }
                                    } else {
                                        Badge { color: Color::Secondary, {tid!("profile-role-member")} }
                                    }
                                }
                                div { class: "col-12",
                                    small { class: "text-muted d-block", {tid!("profile-account-primary-email")} }
                                    span { class: "fw-bold", "{primary_email_str}" }
                                }
                            }
                        },
                    }

                    Card { class: "shadow-sm",
                        header: rsx! {
                            h5 { class: "mb-0",
                                Icon { name: "sliders", class: "me-2" }
                                {tid!("profile-settings-title")}
                            }
                        },
                        body: rsx! {
                            div { class: "d-flex flex-column gap-3",
                                div {
                                    label { class: "form-label fw-bold small text-muted mb-2",
                                        Icon { name: "globe", class: "me-1" }
                                        {tid!("profile-settings-language")}
                                    }
                                    div { class: "d-flex gap-2",
                                        Button {
                                            color: Color::Primary,
                                            outline: current_lang_code != "de",
                                            size: Size::Sm,
                                            onclick: {
                                                let set_fn = set_theme_and_lang.clone();
                                                move |_| {
                                                    let _ = set_fn(None, Some(LanguagePreference::De));
                                                }
                                            },
                                            {tid!("language-switcher-de")}
                                        }
                                        Button {
                                            color: Color::Primary,
                                            outline: current_lang_code != "en",
                                            size: Size::Sm,
                                            onclick: {
                                                let set_fn = set_theme_and_lang.clone();
                                                move |_| {
                                                    let _ = set_fn(None, Some(LanguagePreference::En));
                                                }
                                            },
                                            {tid!("language-switcher-en")}
                                        }
                                    }
                                }

                                hr { class: "my-1" }

                                div {
                                    label { class: "form-label fw-bold small text-muted mb-2",
                                        Icon { name: "circle-half-stroke", class: "me-1" }
                                        {tid!("profile-settings-theme")}
                                    }
                                    div { class: "d-flex gap-2",
                                        Button {
                                            color: Color::Primary,
                                            outline: is_dark,
                                            size: Size::Sm,
                                            onclick: {
                                                let set_fn = set_theme_and_lang.clone();
                                                move |_| {
                                                    let _ = set_fn(Some(ThemePreference::Light), None);
                                                }
                                            },
                                            Icon { name: "sun-fill", class: "me-1" }
                                            {tid!("profile-theme-light")}
                                        }
                                        Button {
                                            color: Color::Primary,
                                            outline: !is_dark,
                                            size: Size::Sm,
                                            onclick: {
                                                let set_fn = set_theme_and_lang.clone();
                                                move |_| {
                                                    let _ = set_fn(Some(ThemePreference::Dark), None);
                                                }
                                            },
                                            Icon { name: "moon-fill", class: "me-1" }
                                            {tid!("profile-theme-dark")}
                                        }
                                    }
                                }
                            }
                        },
                    }
                }

                // Right column: Linked Email Addresses
                Col { md: ColumnSize::Span(6), class: "mb-4",
                    Card { class: "shadow-sm",
                        header: rsx! {
                            h5 { class: "mb-0",
                                Icon { name: "envelope-at", class: "me-2" }
                                {tid!("subscriptions-linked-emails-title")}
                            }
                        },
                        body: rsx! {
                            ul { class: "list-group list-group-flush mb-3",
                                for email in &my_emails {
                                    {
                                        let email_id = email.id;
                                        let is_unverified = !email.is_verified;
                                        let remove_email_for_row = remove_email.clone();
                                        rsx! {
                                            li {
                                                class: "list-group-item d-flex justify-content-between align-items-center px-0",
                                                div {
                                                    class: if is_unverified { "text-muted" } else { "" },
                                                    if email.id == my_primary_email_id {
                                                        strong { "{email.email} " }
                                                        span { class: "text-muted", {tid!("subscriptions-primary-badge")} }
                                                    } else if is_unverified {
                                                        span { class: "fst-italic", "{email.email}" }
                                                    } else {
                                                        span { "{email.email}" }
                                                    }
                                                    if is_unverified {
                                                        span {
                                                            title: tid!("subscriptions-email-unconfirmed-tooltip"),
                                                            Badge {
                                                                color: Color::Warning,
                                                                class: "ms-2",
                                                                {tid!("subscriptions-email-not-confirmed")}
                                                            }
                                                        }
                                                    }
                                                }
                                                div { class: "d-flex align-items-center gap-2",
                                                    if is_unverified {
                                                        {
                                                            let was_resent = resent_email_id() == Some(email_id);
                                                            let add_email_for_resend = add_email.clone();
                                                            let email_addr = email.email.clone();
                                                            let mut resent_sig = resent_email_id;
                                                            rsx! {
                                                                Button {
                                                                    color: if was_resent { Color::Success } else { Color::Primary },
                                                                    outline: true,
                                                                    size: Size::Sm,
                                                                    disabled: was_resent,
                                                                    title: tid!("subscriptions-resend-verification"),
                                                                    onclick: move |_| {
                                                                        if let Err(e) = add_email_for_resend(email_addr.clone()) {
                                                                            ::dioxus::logger::tracing::error!("Failed to resend verification: {e:?}");
                                                                        } else {
                                                                            resent_sig.set(Some(email_id));
                                                                            spawn_local(async move {
                                                                                gloo_timers::future::TimeoutFuture::new(3000).await;
                                                                                resent_sig.set(None);
                                                                            });
                                                                        }
                                                                    },
                                                                    Icon { name: if was_resent { "check2" } else { "envelope-arrow-up" }, class: "me-1" }
                                                                    if was_resent {
                                                                        {tid!("subscriptions-verification-resent")}
                                                                    } else {
                                                                        {tid!("subscriptions-resend-verification")}
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    if email.source != EmailSource::ExternalSync && email.id != my_primary_email_id {
                                                        Button {
                                                            color: Color::Danger,
                                                            size: Size::Sm,
                                                            onclick: move |_| {
                                                                if let Err(e) = remove_email_for_row(email_id) {
                                                                    ::dioxus::logger::tracing::error!("remove_account_email failed: {e:?}");
                                                                }
                                                            },
                                                            Icon { name: "trash" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if show_add_email() {
                                div { class: "d-flex gap-2 align-items-center",
                                    input {
                                        class: "form-control form-control-sm",
                                        r#type: "email",
                                        placeholder: tid!("subscriptions-add-email-placeholder"),
                                        style: "max-width: 300px;",
                                        value: "{add_email_input}",
                                        oninput: move |e: FormEvent| add_email_input.set(e.value()),
                                    }
                                    {
                                        let add_email_for_submit = add_email.clone();
                                        rsx! {
                                            Button {
                                                color: Color::Success,
                                                size: Size::Sm,
                                                onclick: move |_| {
                                                    let email_str = add_email_input();
                                                    if !email_str.is_empty() {
                                                        if let Err(e) = add_email_for_submit(email_str) {
                                                            ::dioxus::logger::tracing::error!("Failed to request email verification: {e:?}");
                                                        } else {
                                                            show_add_email.set(false);
                                                            add_email_input.set(String::new());
                                                        }
                                                    }
                                                },
                                                {tid!("subscriptions-send-verification")}
                                            }
                                        }
                                    }
                                    Button {
                                        color: Color::Secondary,
                                        size: Size::Sm,
                                        onclick: move |_| {
                                            show_add_email.set(false);
                                            add_email_input.set(String::new());
                                        },
                                        {tid!("general-cancel")}
                                    }
                                }
                            } else {
                                Button {
                                    color: Color::Primary,
                                    size: Size::Sm,
                                    onclick: move |_| {
                                        show_add_email.set(true);
                                        add_email_input.set(String::new());
                                    },
                                    Icon { name: "plus-lg", class: "me-2" }
                                    {tid!("subscriptions-add-email-button")}
                                }
                            }
                        },
                    }
                }
            }

            // Advanced & Diagnostics Section
            details { class: "p-3 bg-light rounded border",
                summary { class: "fw-bold text-muted user-select-none", style: "cursor: pointer;",
                    Icon { name: "terminal", class: "me-2" }
                    {tid!("profile-advanced-title")}
                }
                div { class: "mt-3 pt-3 border-top",
                    label { class: "form-label text-muted small fw-bold mb-1",
                        {tid!("profile-advanced-identity")}
                    }
                    div { class: "input-group input-group-sm", style: "max-width: 650px;",
                        input {
                            class: "form-control font-monospace",
                            r#type: "text",
                            readonly: true,
                            value: "{identity_hex}",
                        }
                        {
                            let id_for_copy = identity_hex.clone();
                            rsx! {
                                Button {
                                    color: Color::Secondary,
                                    outline: true,
                                    onclick: move |_| {
                                        let text_to_copy = id_for_copy.clone();
                                        spawn_local(async move {
                                            if let Some(window) = web_sys::window() {
                                                let promise = window.navigator().clipboard().write_text(&text_to_copy);
                                                if JsFuture::from(promise).await.is_ok() {
                                                    copied_identity.set(true);
                                                    gloo_timers::future::TimeoutFuture::new(2000).await;
                                                    copied_identity.set(false);
                                                }
                                            }
                                        });
                                    },
                                    Icon { name: if copied_identity() { "check2" } else { "clipboard" }, class: "me-1" }
                                    if copied_identity() {
                                        {tid!("profile-advanced-copied")}
                                    } else {
                                        {tid!("profile-advanced-copy-id")}
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
