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
    use_reducer_unregister_admin_identity, use_subscription, use_table_visible_admin_identities,
    use_table_visible_domains, use_table_visible_webhook_tokens,
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
fn DomainsCard() -> Element {
    use_subscription(&["SELECT * FROM visible_domains"]);
    let domains = use_table_visible_domains();
    let (sync_domains_invoke, sync_domains_result) = use_procedure_sync_stalwart_domains();

    rsx! {
        Row { class: "mt-4",
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
