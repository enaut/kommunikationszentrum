use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{
    use_reducer_add_subscription, use_reducer_remove_subscription, use_table_message_categories,
    use_table_visible_accounts, use_table_visible_subscriptions,
};

/// Admin-only view: all members with their current subscriptions.
/// Admins can add or remove subscriptions on behalf of any member.
#[component]
pub fn MembersPage() -> Element {
    let accounts = use_table_visible_accounts();
    let subscriptions = use_table_visible_subscriptions();
    let categories = use_table_message_categories();
    let add_subscription = use_reducer_add_subscription();
    let remove_subscription = use_reducer_remove_subscription();

    // Which account's inline add-subscription form is currently open.
    let mut add_form_account: Signal<Option<u64>> = use_signal(|| None);
    // Selected category id in that form (0 = nothing selected).
    let mut add_form_category: Signal<u64> = use_signal(|| 0);

    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "people-fill", class: "me-2" }
                        "Mitglieder"
                    }
                    p { class: "text-muted mt-1",
                        Badge { color: Color::Primary, class: "me-2", "{accounts().len()}" }
                        "registrierte Mitglieder"
                    }
                }
            }

            if accounts().is_empty() {
                Alert { color: Color::Info,
                    Icon { name: "info-circle", class: "me-2" }
                    "Keine Mitglieder gefunden."
                }
            } else {
                Card { class: "shadow-sm", body_class: "p-0",
                    body: rsx! {
                        div { class: "table-responsive",
                            table { class: "table table-hover mb-0",
                                thead { class: "table-light",
                                    tr {
                                        th { "ID" }
                                        th { "Name" }
                                        th { "E-Mail" }
                                        th { "Status" }
                                        th { "Abonnements" }
                                        th { "Aktion" }
                                    }
                                }
                                tbody {
                                    for account in accounts() {
                                        {
                                            let acct_id = account.id;
                                            let acct_email = account.email.clone();
                                            let member_subs: Vec<_> = subscriptions()
                                                .into_iter()
                                                .filter(|s| {
                                                    s.subscriber_account_id == acct_id && s.active
                                                })
                                                .collect();
                                            let is_form_open = add_form_account() == Some(acct_id);
                                            rsx! {
                                                tr {
                                                    td { code { "{account.id}" } }
                                                    td { "{account.name}" }
                                                    td { small { class: "text-muted", "{account.email}" } }
                                                    td {
                                                        if account.is_active {
                                                            Badge { color: Color::Success, "Aktiv" }
                                                        } else {
                                                            Badge { color: Color::Danger, "Inaktiv" }
                                                        }
                                                    }
                                                    td {
                                                        for sub in &member_subs {
                                                            {
                                                                let sub_id = sub.id;
                                                                let cat_name = categories()
                                                                    .into_iter()
                                                                    .find(|c| c.id == sub.category_id)
                                                                    .map(|c| c.name)
                                                                    .unwrap_or_else(|| {
                                                                        format!("#{}", sub.category_id)
                                                                    });
                                                                let remove = remove_subscription.clone();
                                                                rsx! {
                                                                    Badge { color: Color::Primary, class: "me-1 mb-1 d-inline-flex align-items-center gap-1",
                                                                        "{cat_name}"
                                                                        button {
                                                                            class: "btn-close btn-close-white",
                                                                            style: "font-size: 0.5rem;",
                                                                            "aria-label": "Abonnement entfernen",
                                                                            onclick: move |_| {
                                                                                info!(
                                                                                    "Removing subscription {sub_id}"
                                                                                );
                                                                                if let Err(e) = remove(sub_id) {
                                                                                    error!(
                                                                                        "remove_subscription failed: {e:?}"
                                                                                    );
                                                                                }
                                                                            },
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    td {
                                                        if is_form_open {
                                                            div { class: "d-flex gap-2 align-items-center",
                                                                select {
                                                                    class: "form-select form-select-sm",
                                                                    style: "width: auto; min-width: 10rem;",
                                                                    onchange: move |e| {
                                                                        if let Ok(id) = e.value().parse::<u64>() {
                                                                            add_form_category.set(id);
                                                                        }
                                                                    },
                                                                    option { value: "0", "– Kategorie –" }
                                                                    for cat in categories()
                                                                        .into_iter()
                                                                        .filter(|c| c.active)
                                                                    {
                                                                        {
                                                                            let already = member_subs
                                                                                .iter()
                                                                                .any(|s| s.category_id == cat.id);
                                                                            if !already {
                                                                                let val = cat.id.to_string();
                                                                                rsx! {
                                                                                    option { value: "{val}", "{cat.name}" }
                                                                                }
                                                                            } else {
                                                                                rsx! {}
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                {
                                                                    let add = add_subscription.clone();
                                                                    let email_for_add = acct_email.clone();
                                                                    rsx! {
                                                                        Button {
                                                                            color: Color::Success,
                                                                            size: Size::Sm,
                                                                            disabled: add_form_category() == 0,
                                                                            onclick: move |_| {
                                                                                let cat_id = add_form_category();
                                                                                if cat_id == 0 {
                                                                                    return;
                                                                                }
                                                                                info!(
                                                                                    "Adding subscription: account={acct_id}, category={cat_id}"
                                                                                );
                                                                                if let Err(e) = add(
                                                                                    acct_id,
                                                                                    email_for_add.clone(),
                                                                                    cat_id,
                                                                                ) {
                                                                                    error!(
                                                                                        "add_subscription failed: {e:?}"
                                                                                    );
                                                                                } else {
                                                                                    add_form_account.set(None);
                                                                                    add_form_category.set(0);
                                                                                }
                                                                            },
                                                                            Icon { name: "check-lg" }
                                                                        }
                                                                        Button {
                                                                            color: Color::Secondary,
                                                                            size: Size::Sm,
                                                                            onclick: move |_| {
                                                                                add_form_account.set(None);
                                                                                add_form_category.set(0);
                                                                            },
                                                                            Icon { name: "x-lg" }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        } else {
                                                            Button {
                                                                color: Color::Primary,
                                                                outline: true,
                                                                size: Size::Sm,
                                                                onclick: move |_| {
                                                                    add_form_account.set(Some(acct_id));
                                                                    add_form_category.set(0);
                                                                },
                                                                Icon { name: "plus-lg", class: "me-1" }
                                                                "Kategorie"
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
        }
    }
}
