use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::dioxus::{
    use_reducer_admin_add_subscription, use_reducer_update_account_config,
    use_reducer_update_subscription_permission, use_table_total_accounts,
    use_table_visible_account_configs,
};
use crate::module_bindings::{
    Account, AccountEmail, SubscriptionPermission, SubscriptionStatus,
};
use crate::pages::topic::subscribers::{
    parse_status, status_key, status_label, ALL_STATUSES,
};

/// Auto-select threshold: if the filtered list has fewer than this many entries,
/// the first result is selected automatically.
const AUTO_SELECT_THRESHOLD: usize = 20;

/// Target data for editing a subscriber's status in the edit modal.
#[derive(Clone, PartialEq, Debug)]
pub struct EditSubscriptionTarget {
    pub subscription_id: u64,
    pub account_id: u64,
    pub account_email_id: u64,
    pub name: String,
    pub email: String,
    pub status: SubscriptionStatus,
    pub permission: crate::module_bindings::SubscriptionPermission,
}

/// Modal for adding a new subscriber to the topic.
#[component]
pub fn AddSubscriberModal(
    mut show: Signal<bool>,
    topic_id: u64,
    available_accounts: Vec<Account>,
    available_emails: Vec<AccountEmail>,
    #[props(default)]
    subscribed_email_ids: HashSet<u64>,
) -> Element {
    let admin_add_subscription = use_reducer_admin_add_subscription();
    let update_config = use_reducer_update_account_config();
    let total_accounts_table = use_table_total_accounts();
    let account_configs_table = use_table_visible_account_configs();

    let mut selected_account_id = use_signal(|| 0u64);
    let mut selected_email_id = use_signal(|| 0u64);
    let mut account_filter = use_signal(String::new);
    let mut selected_status = use_signal(|| SubscriptionStatus::ManuallySubscribed);
    let mut add_sub_error: Signal<Option<String>> = use_signal(|| None);

    // Save previous member filters to restore when modal closes
    let saved_member_offset = use_hook(|| Rc::new(Cell::new(None::<u32>)));
    let saved_member_search_query = use_hook(|| Rc::new(RefCell::new(None::<Option<String>>)));

    let restore_filters = {
        let update_config = update_config.clone();
        let saved_offset = saved_member_offset.clone();
        let saved_query = saved_member_search_query.clone();
        move || {
            let offset = saved_offset.take();
            let query = saved_query.borrow_mut().take();
            if let Some(offset_val) = offset {
                if let Some(query_opt) = query {
                    match query_opt {
                        Some(q) => {
                            let _ = update_config(None, None, None, false, Some(offset_val), None, Some(q), false, None, false, None, None);
                        }
                        None => {
                            let _ = update_config(None, None, None, false, Some(offset_val), None, None, true, None, false, None, None);
                        }
                    }
                }
            }
        }
    };

    // Track show transitions using a non-reactive Cell to avoid feedback loops
    let was_open = use_hook(|| Cell::new(false));
    use_effect({
        let update_config = update_config.clone();
        let saved_offset = saved_member_offset.clone();
        let saved_query = saved_member_search_query.clone();
        let restore = restore_filters.clone();
        move || {
            let is_open = show();
            let had_open = was_open.get();
            if is_open && !had_open {
                selected_account_id.set(0);
                selected_email_id.set(0);
                account_filter.set(String::new());
                selected_status.set(SubscriptionStatus::ManuallySubscribed);
                add_sub_error.set(None);
                // Snapshot current member filter state
                if let Some(config) = account_configs_table().into_iter().next() {
                    saved_offset.set(Some(config.member_offset));
                    *saved_query.borrow_mut() = Some(config.member_search_query);
                }
                // Clear server-side search query while searching in modal
                let _ = update_config(None, None, None, false, Some(0), None, None, true, None, false, None, None);
            } else if !is_open && had_open {
                // Restore original member filters when closing
                restore();
            }
            was_open.set(is_open);
        }
    });

    use_drop({
        let restore = restore_filters.clone();
        move || {
            restore();
        }
    });

    let filter_lower = account_filter().to_lowercase();
    let trimmed_filter = filter_lower.trim();

    let filtered_accounts: Vec<_> = available_accounts
        .iter()
        .filter(|a| {
            trimmed_filter.is_empty()
                || a.name.to_lowercase().contains(trimmed_filter)
                || a.id.to_string().contains(trimmed_filter)
                || available_emails.iter().any(|e| e.account_id == a.id && e.is_verified && e.email.to_lowercase().contains(trimmed_filter))
        })
        .cloned()
        .collect();

    let total_accounts = total_accounts_table()
        .into_iter()
        .next()
        .map(|r| r.count as usize)
        .unwrap_or(available_accounts.len());

    let filter_count_text = tid!(
        "members-filter-count",
        filtered: filtered_accounts.len(),
        total: total_accounts
    );

    // Determine effective account id purely from state (no side-effect loops)
    let current_acc_id = selected_account_id();
    let effective_account_id = if current_acc_id != 0 && filtered_accounts.iter().any(|a| a.id == current_acc_id) {
        current_acc_id
    } else if !trimmed_filter.is_empty() && filtered_accounts.len() < AUTO_SELECT_THRESHOLD && !filtered_accounts.is_empty() {
        filtered_accounts[0].id
    } else {
        0
    };

    let selectable_emails: Vec<_> = available_emails
        .iter()
        .filter(|e| e.account_id == effective_account_id && e.is_verified && !subscribed_email_ids.contains(&e.id))
        .cloned()
        .collect();

    // Determine effective email id purely from state
    let current_email_id = selected_email_id();
    let effective_email_id = if current_email_id != 0 && selectable_emails.iter().any(|e| e.id == current_email_id) {
        current_email_id
    } else {
        selectable_emails.first().map(|e| e.id).unwrap_or(0)
    };

    rsx! {
        Modal {
            show,
            title: tid!("subscriber-add-title"),
            body: rsx! {
                if let Some(err) = add_sub_error.read().clone() {
                    Alert { color: Color::Danger, class: "mb-3", "{err}" }
                }
                FormGroup { label: tid!("subscriber-member-label"),
                    InputGroup { class: "mb-2",
                        InputGroupText { Icon { name: "search" } }
                        Input {
                            r#type: "search",
                            placeholder: tid!("subscriber-search-placeholder"),
                            value: "{account_filter}",
                            oninput: {
                                let update_config = update_config.clone();
                                move |e: FormEvent| {
                                    let new_val = e.value();
                                    let trimmed = new_val.trim().to_string();
                                    account_filter.set(new_val);
                                    if trimmed.is_empty() {
                                        let _ = update_config(None, None, None, false, Some(0), None, None, true, None, false, None, None);
                                    } else {
                                        let _ = update_config(None, None, None, false, Some(0), None, Some(trimmed), false, None, false, None, None);
                                    }
                                }
                            },
                        }
                    }
                    Select {
                        value: effective_account_id.to_string(),
                        onchange: move |e: FormEvent| {
                            if let Ok(id) = e.value().parse::<u64>() {
                                selected_account_id.set(id);
                                selected_email_id.set(0);
                            }
                        },
                        option { value: "0",
                            if filtered_accounts.is_empty() {
                                {tid!("general-no-results")}
                            } else {
                                {tid!("subscriber-select-member")}
                            }
                        }
                        for acc in filtered_accounts.clone() {
                            option {
                                key: "{acc.id}",
                                value: "{acc.id}",
                                "{acc.name}"
                            }
                        }
                    }
                    if !trimmed_filter.is_empty() {
                        FormText {
                            "{filter_count_text}"
                        }
                    } else if available_accounts.is_empty() {
                        FormText {
                            {tid!("subscriber-add-all-claimed")}
                        }
                    }
                }

                if effective_account_id != 0 {
                    FormGroup { label: tid!("subscriber-email-label"),
                        Select {
                            value: effective_email_id.to_string(),
                            onchange: move |e: FormEvent| {
                                if let Ok(id) = e.value().parse::<u64>() {
                                    selected_email_id.set(id);
                                }
                            },
                            option { value: "0", {tid!("subscriber-select-email")} }
                            for email in selectable_emails {
                                option {
                                    key: "{email.id}",
                                    value: "{email.id}",
                                    "{email.email}"
                                }
                            }
                        }
                    }
                }

                FormGroup { label: tid!("subscriber-status-label"),
                    Select {
                        value: status_key(&selected_status()).to_string(),
                        onchange: move |e: FormEvent| {
                            if let Some(s) = parse_status(&e.value()) {
                                selected_status.set(s);
                            }
                        },
                        for s in ALL_STATUSES {
                            option {
                                key: "{status_key(s)}",
                                value: "{status_key(s)}",
                                "{status_label(s)}"
                            }
                        }
                    }
                }
            },
            footer: rsx! {
                Button {
                    color: Color::Secondary,
                    onclick: {
                        let update_config = update_config.clone();
                        move |_| {
                            let _ = update_config(None, None, None, false, Some(0), None, None, true, None, false, None, None);
                            show.set(false);
                        }
                    },
                    {tid!("subscriber-cancel")}
                }
                Button {
                    color: Color::Primary,
                    disabled: effective_account_id == 0 || effective_email_id == 0,
                    onclick: {
                        let update_config = update_config.clone();
                        move |_| {
                            let acc_id = effective_account_id;
                            let email_id = effective_email_id;
                            if acc_id == 0 || email_id == 0 {
                                return;
                            }
                            let status = selected_status();
                            info!(
                                "Admin adding subscription: account={acc_id}, email_id={email_id}, topic={topic_id}, status={status:?}"
                            );
                            match admin_add_subscription(acc_id, email_id, topic_id, status) {
                                Ok(()) => {
                                    let _ = update_config(None, None, None, false, Some(0), None, None, true, None, false, None, None);
                                    show.set(false);
                                }
                                Err(e) => {
                                    error!("admin_add_subscription failed: {e:?}");
                                    add_sub_error.set(Some(tid!("subscriber-error", error: format!("{e:?}"))));
                                }
                            }
                        }
                    },
                    Icon { name: "check-lg", class: "me-2" }
                    {tid!("subscriber-add")}
                }
            },
        }
    }
}

/// Modal for editing an existing subscriber's status.
#[component]
pub fn EditSubscriptionModal(
    mut show: Signal<bool>,
    topic_id: u64,
    target: Signal<Option<EditSubscriptionTarget>>,
) -> Element {
    let admin_add_subscription = use_reducer_admin_add_subscription();
    let update_permission = use_reducer_update_subscription_permission();

    let mut edit_status = use_signal(|| SubscriptionStatus::ManuallySubscribed);
    let mut edit_permission = use_signal(|| SubscriptionPermission::Read);
    let mut edit_sub_error: Signal<Option<String>> = use_signal(|| None);

    // Sync state when target changes
    use_effect(move || {
        if let Some(t) = target() {
            edit_status.set(t.status);
            edit_permission.set(t.permission);
            edit_sub_error.set(None);
        }
    });

    let target_val = target();
    let account_name = target_val
        .as_ref()
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let account_email = target_val
        .as_ref()
        .map(|t| t.email.clone())
        .unwrap_or_default();

    rsx! {
        Modal {
            show,
            title: tid!("subscriber-edit-title"),
            body: rsx! {
                if let Some(err) = edit_sub_error.read().clone() {
                    Alert { color: Color::Danger, class: "mb-3", "{err}" }
                }
                FormGroup { label: tid!("subscriber-member-label"),
                    Input {
                        r#type: "text",
                        value: "{account_name}",
                        disabled: true,
                        readonly: true,
                    }
                }
                FormGroup { label: tid!("subscriber-email-label"),
                    Input {
                        r#type: "text",
                        value: "{account_email}",
                        disabled: true,
                        readonly: true,
                    }
                }
                FormGroup { label: tid!("subscriber-status-label"),
                    Select {
                        value: status_key(&edit_status()).to_string(),
                        onchange: move |e: FormEvent| {
                            if let Some(s) = parse_status(&e.value()) {
                                edit_status.set(s);
                            }
                        },
                        for s in ALL_STATUSES {
                            option {
                                key: "{status_key(s)}",
                                value: "{status_key(s)}",
                                "{status_label(s)}"
                            }
                        }
                    }
                }
                FormGroup { label: tid!("subscriber-permission-label"),
                    Select {
                        value: match edit_permission() {
                            SubscriptionPermission::Read => "read",
                            SubscriptionPermission::Write => "write",
                        },
                        onchange: move |e: FormEvent| {
                            match e.value().as_str() {
                                "read" => edit_permission.set(SubscriptionPermission::Read),
                                "write" => edit_permission.set(SubscriptionPermission::Write),
                                _ => {}
                            }
                        },
                        option { value: "read", {tid!("subscriber-permission-read")} }
                        option { value: "write", {tid!("subscriber-permission-write")} }
                    }
                }
            },
            footer: rsx! {
                Button {
                    color: Color::Secondary,
                    onclick: move |_| show.set(false),
                    {tid!("subscriber-cancel")}
                }
                Button {
                    color: Color::Primary,
                    onclick: move |_| {
                        let Some(t) = target() else { return };
                        let status = edit_status();
                        let permission = edit_permission();
                        info!(
                            "Admin updating subscription: account={}, topic={topic_id}, status={status:?}, permission={permission:?}",
                            t.account_id
                        );
                        
                        let res1 = admin_add_subscription(t.account_id, t.account_email_id, topic_id, status);
                        let res2 = update_permission(t.subscription_id, permission);
                        
                        if res1.is_err() || res2.is_err() {
                            error!("admin_add_subscription or update_permission failed");
                            edit_sub_error.set(Some(tid!("subscriber-error", error: format!("{:?}", res1.err().or(res2.err())))));
                        } else {
                            show.set(false);
                        }
                    },
                    Icon { name: "check-lg", class: "me-2" }
                    {tid!("subscriber-save")}
                }
            },
        }
    }
}
