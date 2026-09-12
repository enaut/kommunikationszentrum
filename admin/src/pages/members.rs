use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::{
    module_bindings::dioxus::{
        use_reducer_admin_add_account_email, use_reducer_admin_add_subscription,
        use_reducer_remove_account_email, use_reducer_remove_subscription, use_subscription,
        use_table_visible_account_emails, use_table_visible_accounts,
        use_table_visible_message_categories, use_table_visible_subscriptions,
        use_table_visible_account_configs, use_reducer_update_account_config,
        use_table_total_accounts,
    },
    module_bindings::{EmailSource, SubscriptionStatus},
    oauth::UserInfo,
    pages::category::status_color,
};

/// Admin-only view: all members with their current subscriptions.
/// Admins can add or remove subscriptions on behalf of any member.
#[component]
pub fn MembersPage(user_info: UserInfo) -> Element {
    use_subscription(&[
        "SELECT * FROM visible_accounts",
        "SELECT * FROM visible_account_emails",
        "SELECT * FROM visible_message_categories",
        "SELECT * FROM visible_subscriptions",
        "SELECT * FROM visible_account_configs",
        "SELECT * FROM total_accounts",
    ]);
    let accounts = use_table_visible_accounts();
    let account_emails = use_table_visible_account_emails();
    let subscriptions = use_table_visible_subscriptions();
    let categories = use_table_visible_message_categories();
    let configs = use_table_visible_account_configs();
    let total_accounts_table = use_table_total_accounts();
    let update_config = use_reducer_update_account_config();
    let add_subscription = use_reducer_admin_add_subscription();
    let remove_subscription = use_reducer_remove_subscription();

    // Which account's inline add-subscription form is currently open.
    let mut add_form_account: Signal<Option<u64>> = use_signal(|| None);
    // Selected category id in that form (0 = nothing selected).
    let mut add_form_category: Signal<u64> = use_signal(|| 0);
    // Selected account_email_id in that form.
    let mut add_form_email_id: Signal<u64> = use_signal(|| 0);

    let admin_add_email = use_reducer_admin_add_account_email();
    let remove_email = use_reducer_remove_account_email();
    
    // Which account's inline add-email form is currently open.
    let mut add_email_account: Signal<Option<u64>> = use_signal(|| None);
    let mut add_email_input: Signal<String> = use_signal(|| String::new());

    let config = configs().into_iter().next();
    let search_query = config.as_ref().and_then(|c| c.member_search_query.clone()).unwrap_or_default();
    let current_offset = config.as_ref().map(|c| c.member_offset).unwrap_or(0);
    let current_limit = config.as_ref().map(|c| c.member_limit).unwrap_or(50);

    let total_accounts = total_accounts_table()
        .into_iter()
        .next()
        .map(|r| r.count as u32)
        .unwrap_or_else(|| accounts().len() as u32);
    let search_matching_accounts = config
        .as_ref()
        .map(|c| c.search_matching_accounts)
        .unwrap_or(total_accounts);

    let all_accounts = accounts();

    // filtered_accounts is now just all_accounts because the server already filters and slices
    let mut filtered_accounts = all_accounts.clone();
    filtered_accounts.sort_by_key(|a| a.id);

    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3 align-items-center",
                Col { md: ColumnSize::Span(6),
                    h2 { class: "mb-0",
                        Icon { name: "people-fill", class: "me-2" }
                        {tid!("members-page-title")}
                    }
                    p { class: "text-muted mt-1 mb-0",
                        Badge { color: Color::Primary, class: "me-2", "{search_matching_accounts} / {total_accounts}" }
                        {tid!("members-summary")}
                    }
                }
                Col { md: ColumnSize::Span(6), class: "mt-2 mt-md-0",
                    div { class: "d-flex align-items-center gap-2",
                        InputGroup { class: "mb-0",
                            InputGroupText { Icon { name: "search" } }
                            Input {
                                r#type: "search",
                                placeholder: tid!("subscriber-search-placeholder"),
                                value: "{search_query}",
                                oninput: {
                                    let update_config = update_config.clone();
                                    move |e: FormEvent| {
                                        let val = e.value();
                                        if val.is_empty() {
                                            update_config(None, None, None, false, Some(0), None, None, true, None, false, None, None);
                                        } else {
                                            update_config(None, None, None, false, Some(0), None, Some(val), false, None, false, None, None);
                                        }
                                    }
                                },
                            }
                        }
                        div { class: "ms-auto d-flex align-items-center gap-2 text-nowrap",
                            Button {
                                color: Color::Secondary,
                                outline: true,
                                size: Size::Sm,
                                disabled: current_offset == 0,
                                onclick: {
                                    let update_config = update_config.clone();
                                    move |_| {
                                        if current_offset >= current_limit {
                                            update_config(None, None, None, false, Some(current_offset - current_limit), None, None, false, None, false, None, None);
                                        } else {
                                            update_config(None, None, None, false, Some(0), None, None, false, None, false, None, None);
                                        }
                                    }
                                },
                                Icon { name: "chevron-left" }
                            }
                            span { class: "text-muted small",
                                "Page {(current_offset / current_limit) + 1}"
                            }
                            Button {
                                color: Color::Secondary,
                                outline: true,
                                size: Size::Sm,
                                disabled: current_offset + current_limit >= search_matching_accounts,
                                onclick: {
                                    let update_config = update_config.clone();
                                    move |_| {
                                        update_config(None, None, None, false, Some(current_offset + current_limit), None, None, false, None, false, None, None);
                                    }
                                },
                                Icon { name: "chevron-right" }
                            }
                        }
                    }
                }
            }

            if filtered_accounts.is_empty() {
                Alert { color: Color::Info,
                    Icon { name: "info-circle", class: "me-2" }
                    {tid!("members-empty")}
                }
            } else {
                Card {
                    class: "shadow-sm",
                    body_class: "p-0",
                    body: rsx! {
                        Table { hover: true, responsive: true, class: "mb-0",
                            thead { class: "table-light",
                                tr {
                                    th { {tid!("members-table-id")} }
                                    th { {tid!("members-table-name")} }
                                    th { {tid!("members-table-email")} }
                                    th { {tid!("members-table-status")} }
                                    th { {tid!("members-table-subscriptions")} }
                                    th { {tid!("members-table-action")} }
                                }
                            }
                            tbody {
                                for account in filtered_accounts {
                                    {
                                        let acct_id = account.id;
                                        let primary_email_id = account.primary_email_id;
                                        let emails: Vec<_> = account_emails()
                                            .into_iter()
                                            .filter(|e| e.account_id == acct_id)
                                            .collect();
                                        let member_subs: Vec<_> = subscriptions()
                                            .into_iter()
                                            .filter(|s| {
                                                s.subscriber_account_id == acct_id
                                                    && crate::pages::is_active_subscription(&s.status)
                                            })
                                            .collect();
                                        let is_form_open = add_form_account() == Some(acct_id);
                                        rsx! {
                                            tr {
                                                td {
                                                    code { "{account.id}" }
                                                }
                                                td { "{account.name}" }
                                                td {
                                                    div { class: "d-flex flex-column gap-1",
                                                        for email in &emails {
                                                            {
                                                                let email_id = email.id;
                                                                let remove_email_for_row = remove_email.clone();
                                                                rsx! {
                                                                    div { class: "d-flex align-items-center gap-1",
                                                                        small { class: "text-muted",
                                                                            if email.id == primary_email_id {
                                                                                strong { "{email.email}" }
                                                                            } else {
                                                                                "{email.email}"
                                                                            }
                                                                        }
                                                                        if email.source != EmailSource::DjangoSync && email.id != primary_email_id {
                                                                            button {
                                                                                class: "btn-close text-danger ms-auto",
                                                                                style: "font-size: 0.5rem;",
                                                                                "aria-label": tid!("members-remove-email"),
                                                                                onclick: move |_| {
                                                                                    if let Err(e) = remove_email_for_row(email_id) {
                                                                                        error!("Failed to remove email: {e:?}");
                                                                                    }
                                                                                },
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        if add_email_account() == Some(acct_id) {
                                                            div { class: "d-flex gap-1 mt-1",
                                                                input {
                                                                    class: "form-control form-control-sm",
                                                                    r#type: "email",
                                                                    placeholder: tid!("members-add-email-placeholder"),
                                                                    value: "{add_email_input}",
                                                                    oninput: move |e: FormEvent| add_email_input.set(e.value()),
                                                                }
                                                                {
                                                                    let admin_add_email_for_row = admin_add_email.clone();
                                                                    rsx! {
                                                                        Button {
                                                                            color: Color::Success,
                                                                            size: Size::Sm,
                                                                            onclick: move |_| {
                                                                                let email_str = add_email_input();
                                                                                if !email_str.is_empty() {
                                                                                    if let Err(e) = admin_add_email_for_row(acct_id, email_str) {
                                                                                        error!("Failed to add email: {e:?}");
                                                                                    } else {
                                                                                        add_email_account.set(None);
                                                                                        add_email_input.set(String::new());
                                                                                    }
                                                                                }
                                                                            },
                                                                            Icon { name: "check-lg" }
                                                                        }
                                                                    }
                                                                }
                                                                Button {
                                                                    color: Color::Secondary,
                                                                    size: Size::Sm,
                                                                    onclick: move |_| {
                                                                        add_email_account.set(None);
                                                                        add_email_input.set(String::new());
                                                                    },
                                                                    Icon { name: "x-lg" }
                                                                }
                                                            }
                                                        } else {
                                                            button {
                                                                class: "btn btn-link btn-sm p-0 text-start mt-1",
                                                                style: "font-size: 0.8rem;",
                                                                onclick: move |_| {
                                                                    add_email_account.set(Some(acct_id));
                                                                    add_email_input.set(String::new());
                                                                },
                                                                {tid!("members-add-email")}
                                                            }
                                                        }
                                                    }
                                                }
                                                td {
                                                    if account.is_active {
                                                        Badge { color: Color::Success, {tid!("members-status-active")} }
                                                    } else {
                                                        Badge { color: Color::Danger, {tid!("members-status-inactive")} }
                                                    }
                                                }
                                                td {
                                                    for sub in &member_subs {
                                                        {
                                                            let sub_id = sub.id;
                                                            let cat_color = status_color(&sub.status);
                                                            let cat = categories()
                                                                .into_iter()
                                                                .find(|c| c.id == sub.category_id);
                                                            let cat_name = cat
                                                                .map(|c| c.name)
                                                                .unwrap_or_else(|| { format!("#{}", sub.category_id) });
                                                            let email = emails.iter().find(|e| e.id == sub.account_email_id).map(|e| e.email.clone()).unwrap_or_default();
                                                            let display_name = format!("{} ({})", cat_name, email);
                                                            let remove = remove_subscription.clone();
                                                            rsx! {
                                                                Badge {
                                                                    color: cat_color,
                                                                    class: "me-1 mb-1 d-inline-flex align-items-center gap-1",
                                                                    "{display_name}"
                                                                    button {
                                                                        class: "btn-close btn-close-white",
                                                                        style: "font-size: 0.5rem;",
                                                                        "aria-label": tid!("members-remove-subscription"),
                                                                        onclick: move |_| {
                                                                            info!("Removing subscription {sub_id}");
                                                                            if let Err(e) = remove(sub_id) {
                                                                                error!("remove_subscription failed: {e:?}");
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
                                                        div { class: "d-flex gap-2 align-items-center flex-wrap",
                                                            Select {
                                                                size: Size::Sm,
                                                                style: "width: auto; min-width: 10rem;",
                                                                onchange: move |e: FormEvent| {
                                                                    if let Ok(id) = e.value().parse::<u64>() {
                                                                        add_form_category.set(id);
                                                                    }
                                                                },
                                                                option { value: "0", {tid!("general-no-topic-selected")} }
                                                                for cat in categories().into_iter().filter(|c| c.active) {
                                                                    {
                                                                        let val = cat.id.to_string();
                                                                        rsx! {
                                                                            option { value: "{val}", "{cat.name}" }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Select {
                                                                size: Size::Sm,
                                                                style: "width: auto; min-width: 10rem;",
                                                                onchange: move |e: FormEvent| {
                                                                    if let Ok(id) = e.value().parse::<u64>() {
                                                                        add_form_email_id.set(id);
                                                                    }
                                                                },
                                                                for email in &emails {
                                                                    option {
                                                                        value: "{email.id}",
                                                                        selected: email.id == primary_email_id,
                                                                        "{email.email}"
                                                                    }
                                                                }
                                                            }
                                                            {
                                                                let add = add_subscription.clone();
                                                                rsx! {
                                                                    Button {
                                                                        color: Color::Success,
                                                                        size: Size::Sm,
                                                                        disabled: add_form_category() == 0,
                                                                        onclick: move |_| {
                                                                            let cat_id = add_form_category();
                                                                            let mut email_id = add_form_email_id();
                                                                            if email_id == 0 {
                                                                                email_id = primary_email_id;
                                                                            }
                                                                            if cat_id == 0 {
                                                                                return;
                                                                            }
                                                                            info!("Adding subscription: account={acct_id}, email_id={email_id}, category={cat_id}");
                                                                            if let Err(e) = add(acct_id, email_id, cat_id, SubscriptionStatus::ManuallySubscribed) {
                                                                                error!("add_subscription failed: {e:?}");
                                                                            } else {
                                                                                add_form_account.set(None);
                                                                                add_form_category.set(0);
                                                                                add_form_email_id.set(0);
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
                                                                            add_form_email_id.set(0);
                                                                        },
                                                                        Icon { name: "x-lg" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        Button {
                                                            color: Color::Primary,
                                                            size: Size::Sm,
                                                            onclick: move |_| {
                                                                add_form_account.set(Some(acct_id));
                                                                add_form_category.set(0);
                                                                add_form_email_id.set(0);
                                                            },
                                                            Icon { name: "plus-lg", class: "me-1" }
                                                            {tid!("members-add-topic")}
                                                        }
                                                    }
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
