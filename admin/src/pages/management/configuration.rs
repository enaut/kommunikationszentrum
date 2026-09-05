use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use crate::module_bindings::dioxus::{
    use_procedure_sync_stalwart_domains, use_reducer_create_webhook_token,
    use_reducer_register_admin_identity, use_reducer_revoke_webhook_token,
    use_reducer_set_stalwart_config_then, use_reducer_unregister_admin_identity, use_subscription,
    use_table_admin_stalwart_config, use_table_visible_admin_identities, use_table_visible_domains,
    use_table_visible_webhook_tokens,
};

#[component]
pub fn ManagementConfigurationPage() -> Element {
    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "sliders", class: "me-2" }
                        "{tid!(\"management-config-title\") }"
                    }
                }
            }
            AdminIdentityCard {}
            WebhookTokenCard {}
            StalwartConfigCard {}
            DomainsCard {}
        }
    }
}

#[component]
fn AdminIdentityCard() -> Element {
    use_subscription(&["SELECT * FROM visible_admin_identities"]);
    let admin_identities = use_table_visible_admin_identities();
    let register_admin = use_reducer_register_admin_identity();
    let unregister_admin = use_reducer_unregister_admin_identity();
    let mut register_hex = use_signal(String::new);

    rsx! {
        Row { class: "mb-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-primary text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "shield-fill", class: "me-2" }
                            "{tid!(\"management-config-admin-identities\") }"
                            span { class: "badge bg-white text-primary ms-2",
                                "{admin_identities().len()}"
                            }
                        }
                    },
                    body: rsx! {
                        Row { class: "g-2 mb-3",
                            Col {
                                Input {
                                    r#type: "text",
                                    size: Size::Sm,
                                    class: "font-monospace",
                                    placeholder: tid!("management-config-admin-identity-placeholder"),
                                    value: "{register_hex}",
                                    oninput: move |e: FormEvent| register_hex.set(e.value()),
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
                                    "{tid!(\"management-config-admin-add\") }"
                                }
                            }
                        }
                        if admin_identities().is_empty() {
                            p { class: "text-muted mb-0", "{tid!(\"management-config-admin-no-identities\") }" }
                        } else {
                            ListGroup { flush: true,
                                for ident in admin_identities() {
                                    {
                                        let hex = ident.identity.to_string();
                                        let hex_for_remove = hex.clone();
                                        let unregister = unregister_admin.clone();
                                        rsx! {
                                            ListGroupItem { tag: "div", class: "d-flex justify-content-between align-items-center",
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
    }
}

#[component]
fn WebhookTokenCard() -> Element {
    use_subscription(&["SELECT * FROM visible_webhook_tokens"]);
    let admin_tokens = use_table_visible_webhook_tokens();
    let create_webhook_token = use_reducer_create_webhook_token();
    let revoke_webhook_token = use_reducer_revoke_webhook_token();
    let mut token_plain = use_signal(String::new);
    let mut token_hash = use_signal(String::new);
    let mut token_label = use_signal(String::new);
    let mut token_copy_button_label = use_signal(|| tid!("management-config-token-copy"));
    let mut permissions_input = use_signal(String::new);

    rsx! {
        Row { class: "mb-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-primary text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "key-fill", class: "me-2" }
                            "{tid!(\"management-config-webhook-title\") }"
                        }
                    },
                    body: rsx! {
                        Row { class: "g-2 mb-3",
                            Col {
                                Input {
                                    r#type: "text",
                                    size: Size::Sm,
                                    placeholder: tid!("management-config-webhook-label"),
                                    value: "{token_label}",
                                    oninput: move |e: FormEvent| token_label.set(e.value()),
                                }
                            }
                            Col {
                                Input {
                                    r#type: "text",
                                    size: Size::Sm,
                                    placeholder: tid!("management-config-webhook-permissions"),
                                    value: "{permissions_input}",
                                    oninput: move |e: FormEvent| permissions_input.set(e.value()),
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
                                    "{tid!(\"management-config-token-generate\") }"
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
                                                        token_copy_button_label.set(tid!("management-config-token-copied"));
                                                        info!("Token copied to clipboard")
                                                    },
                                                    Err(e) => {
                                                        token_copy_button_label.set(tid!("management-config-token-copy-failed"));
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
                            "{tid!(\"management-config-token-create\") }"
                        }

                        if !admin_tokens().is_empty() {
                            ListGroup { flush: true, tag: "div", class: "mt-3",
                                for t in admin_tokens() {
                                    {
                                        let hash = t.token_hash.clone();
                                        let label = t.label.clone();
                                        let perms = t.permissions.join(", ");
                                        let revoke = revoke_webhook_token.clone();
                                        rsx! {
                                            ListGroupItem { tag: "div", class: "d-flex justify-content-between align-items-start",
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
                            p { class: "text-muted mb-0", "{tid!(\"management-config-token-empty\") }" }
                        }

                        p { class: "small text-muted mt-2", "{tid!(\"management-config-token-security\") }" }
                    }
                }
            }
        }
    }
}

#[component]
fn StalwartConfigCard() -> Element {
    use_subscription(&["SELECT * FROM admin_stalwart_config"]);
    let stalwart_configs = use_table_admin_stalwart_config();
    let (set_stalwart_config, save_result) = use_reducer_set_stalwart_config_then();

    let mut jmap_url = use_signal(String::new);
    let mut admin_token = use_signal(String::new);
    let mut show_token = use_signal(|| false);
    let mut is_saving = use_signal(|| false);

    // Synchronize form values whenever received from SpacetimeDB
    use_effect(move || {
        let configs = stalwart_configs();
        if let Some(config) = configs.first() {
            jmap_url.set(config.jmap_url.clone());
            admin_token.set(config.admin_token.clone());
        }
    });

    // Reset is_saving when reducer returns a result
    use_effect(move || {
        if save_result().is_some() {
            is_saving.set(false);
        }
    });

    let current_config = stalwart_configs().first().cloned();
    let is_configured = current_config
        .as_ref()
        .map(|c| !c.jmap_url.trim().is_empty())
        .unwrap_or(false);

    let is_dirty = use_memo(move || {
        let configs = stalwart_configs();
        let server_cfg = configs.first();
        let server_url = server_cfg.map(|c| c.jmap_url.as_str()).unwrap_or("");
        let server_token = server_cfg.map(|c| c.admin_token.as_str()).unwrap_or("");
        jmap_url.read().trim().trim_end_matches('/') != server_url.trim_end_matches('/')
            || admin_token.read().trim() != server_token
    });

    rsx! {
        Row { class: "mb-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-primary text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0 d-flex justify-content-between align-items-center",
                            span {
                                Icon { name: "server", class: "me-2" }
                                "{tid!(\"management-config-stalwart-title\") }"
                                if is_configured {
                                    Badge {
                                        color: Color::Light,
                                        fill: BadgeFill::Bg,
                                        class: "text-success ms-2",
                                        Icon { name: "check-circle-fill", class: "me-1" }
                                        "{tid!(\"management-config-stalwart-status-configured\") }"
                                    }
                                } else {
                                    Badge {
                                        color: Color::Light,
                                        fill: BadgeFill::Bg,
                                        class: "text-warning ms-2",
                                        Icon { name: "exclamation-triangle-fill", class: "me-1" }
                                        "{tid!(\"management-config-stalwart-status-not-configured\") }"
                                    }
                                }
                            }
                        }
                    },
                    body: rsx! {
                        p { class: "text-muted mb-3",
                            Icon { name: "info-circle", class: "me-2" }
                            "{tid!(\"management-config-stalwart-help\") }"
                        }

                        if let Some(result) = save_result() {
                            match result {
                                Ok(()) => rsx! {
                                    Alert {
                                        color: Color::Success,
                                        class: "mb-3 d-flex align-items-center",
                                        Icon { name: "check-circle", class: "me-2 flex-shrink-0" }
                                        span { "{tid!(\"management-config-stalwart-saved\") }" }
                                    }
                                },
                                Err(ref err) => rsx! {
                                    Alert {
                                        color: Color::Danger,
                                        class: "mb-3 d-flex align-items-center",
                                        Icon { name: "exclamation-circle", class: "me-2 flex-shrink-0" }
                                        span { "{err}" }
                                    }
                                },
                            }
                        }

                        Row { class: "g-3 mb-3",
                            Col { md: ColumnSize::Span(6),
                                FormGroup {
                                    label: tid!("management-config-stalwart-jmap-url"),
                                    InputGroup {
                                        InputGroupText { Icon { name: "globe" } }
                                        Input {
                                            r#type: "url",
                                            placeholder: tid!("management-config-stalwart-jmap-url-placeholder"),
                                            value: "{jmap_url}",
                                            oninput: move |e: FormEvent| jmap_url.set(e.value()),
                                        }
                                    }
                                    div { class: "form-text text-muted",
                                        "{tid!(\"management-config-stalwart-jmap-url-help\") }"
                                    }
                                }
                            }
                            Col { md: ColumnSize::Span(6),
                                FormGroup {
                                    label: tid!("management-config-stalwart-admin-token"),
                                    InputGroup {
                                        InputGroupText { Icon { name: "key" } }
                                        Input {
                                            r#type: if show_token() { "text" } else { "password" },
                                            class: "font-monospace",
                                            placeholder: tid!("management-config-stalwart-admin-token-placeholder"),
                                            value: "{admin_token}",
                                            oninput: move |e: FormEvent| admin_token.set(e.value()),
                                        }
                                        Button {
                                            color: Color::Secondary,
                                            outline: true,
                                            title: tid!("management-config-stalwart-toggle-token-visibility"),
                                            onclick: move |_| show_token.set(!show_token()),
                                            Icon { name: if show_token() { "eye-slash" } else { "eye" } }
                                        }
                                    }
                                    div { class: "form-text text-muted",
                                        "{tid!(\"management-config-stalwart-admin-token-help\") }"
                                    }
                                }
                            }
                        }

                        div { class: "d-flex justify-content-between align-items-center flex-wrap gap-2",
                            div { class: "d-flex gap-2",
                                Button {
                                    color: Color::Primary,
                                    size: Size::Sm,
                                    disabled: is_saving()
                                        || jmap_url.read().trim().is_empty()
                                        || admin_token.read().trim().is_empty(),
                                    onclick: {
                                        let save = set_stalwart_config.clone();
                                        move |_| {
                                            let url = jmap_url.read().trim().to_string();
                                            let token = admin_token.read().trim().to_string();
                                            info!("Saving Stalwart config: url={url}");
                                            is_saving.set(true);
                                            save(url, token);
                                        }
                                    },
                                    if is_saving() {
                                        Spinner { size: Size::Sm, class: "me-1" }
                                        "{tid!(\"management-config-stalwart-saving\") }"
                                    } else {
                                        Icon { name: "check-lg", class: "me-1" }
                                        "{tid!(\"management-config-stalwart-save\") }"
                                    }
                                }
                                if is_dirty() {
                                    Button {
                                        color: Color::Secondary,
                                        outline: true,
                                        size: Size::Sm,
                                        onclick: {
                                            let configs = stalwart_configs;
                                            move |_| {
                                                if let Some(config) = configs().first() {
                                                    jmap_url.set(config.jmap_url.clone());
                                                    admin_token.set(config.admin_token.clone());
                                                } else {
                                                    jmap_url.set(String::new());
                                                    admin_token.set(String::new());
                                                }
                                            }
                                        },
                                        Icon { name: "arrow-counterclockwise", class: "me-1" }
                                        "{tid!(\"management-config-stalwart-reset\") }"
                                    }
                                }
                            }
                            if let Some(ref config) = current_config {
                                span { class: "text-muted small",
                                    Icon { name: "clock-history", class: "me-1" }
                                    "{tid!(\"management-config-stalwart-last-updated\") }: "
                                    code { "{config.updated_at.to_string()}" }
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
fn DomainsCard() -> Element {
    use_subscription(&["SELECT * FROM visible_domains"]);
    let domains = use_table_visible_domains();
    let (sync_domains_invoke, sync_domains_result) = use_procedure_sync_stalwart_domains();

    rsx! {
        Row { class: "mb-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-primary text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0 d-flex justify-content-between align-items-center",
                            span {
                                Icon { name: "globe", class: "me-2" }
                                "{tid!(\"management-config-domains-title\") }"
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
                                "{tid!(\"management-config-domains-sync\") }"
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
                                            "{tid!(\"management-config-sync-success\") }"
                                            ": "
                                            strong { "{r.domains_found}" } " {tid!(\"management-config-sync-found\")} , "
                                            strong { "{r.domains_added}" } " {tid!(\"management-config-sync-added\")} , "
                                            strong { "{r.domains_updated}" } " {tid!(\"management-config-sync-updated\")} , "
                                            strong { "{r.domains_removed}" } " {tid!(\"management-config-sync-removed\")} ."
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
                                "{tid!(\"management-config-domains-empty\") }"
                            }
                        } else {
                            Table { hover: true, responsive: true, class: "mb-0",
                                    thead { class: "table-light",
                                        tr {
                                            th { "{tid!(\"management-config-table-id\")}" }
                                            th { "{tid!(\"management-config-table-name\")}" }
                                            th { "{tid!(\"management-config-table-description\")}" }
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
                                                        "{tid!(\"general-none\") }"
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
}
