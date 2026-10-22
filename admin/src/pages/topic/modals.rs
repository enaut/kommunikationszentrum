use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::dioxus::{
    use_reducer_admin_add_subscription, use_reducer_update_account_config,
    use_reducer_update_subscription_permission, use_table_subscriber_search_candidates,
    use_table_visible_account_configs,
};
use crate::module_bindings::{SubscriptionPermission, SubscriptionStatus};
use crate::pages::topic::subscribers::{
    ALL_STATUSES, MODERATOR_STATUSES, parse_status, status_key, status_label,
};
use crate::router::use_is_admin;

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
pub fn AddSubscriberModal(mut show: Signal<bool>, topic_id: u64) -> Element {
    let admin_add_subscription = use_reducer_admin_add_subscription();
    let update_config = use_reducer_update_account_config();
    let candidates_table = use_table_subscriber_search_candidates();
    let account_configs_table = use_table_visible_account_configs();
    let is_admin = use_is_admin();

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
                            let _ = update_config(
                                None,
                                None,
                                None,
                                false,
                                Some(offset_val),
                                None,
                                Some(q),
                                false,
                                None,
                                false,
                                None,
                                None,
                                None,
                                false,
                            );
                        }
                        None => {
                            let _ = update_config(
                                None,
                                None,
                                None,
                                false,
                                Some(offset_val),
                                None,
                                None,
                                true,
                                None,
                                false,
                                None,
                                None,
                                None,
                                false,
                            );
                        }
                    }
                }
            }
        }
    };

    // Track show transitions using a non-reactive Cell to avoid feedback loops
    let was_open = use_hook(|| Rc::new(Cell::new(false)));
    use_effect({
        let update_config = update_config.clone();
        let saved_offset = saved_member_offset.clone();
        let saved_query = saved_member_search_query.clone();
        let restore = restore_filters.clone();
        let was_open = was_open.clone();
        move || {
            let is_open = show();
            let had_open = was_open.get();
            if is_open && !had_open {
                selected_account_id.set(0);
                selected_email_id.set(0);
                account_filter.set(String::new());
                selected_status.set(SubscriptionStatus::ManuallySubscribed);
                add_sub_error.set(None);
                // Snapshot current member filter state without registering reactive dependency
                if let Some(config) = account_configs_table.peek().first() {
                    saved_offset.set(Some(config.member_offset));
                    *saved_query.borrow_mut() = Some(config.member_search_query.clone());
                }
                // Clear server-side search query while searching in modal
                let _ = update_config(
                    None,
                    None,
                    None,
                    false,
                    Some(0),
                    None,
                    None,
                    true,
                    None,
                    false,
                    None,
                    None,
                    None,
                    false,
                );
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

    let candidates = candidates_table();
    let filter_lower = account_filter().to_lowercase();
    let trimmed_filter = filter_lower.trim();

    let mut unique_accounts: Vec<(u64, String)> = Vec::new();
    for c in &candidates {
        if !unique_accounts.iter().any(|(id, _)| *id == c.account_id) {
            unique_accounts.push((c.account_id, c.name.clone()));
        }
    }

    let filter_count_text = tid!(
        "members-filter-count",
        filtered: unique_accounts.len(),
        total: unique_accounts.len()
    );

    // Determine effective account id purely from state (no side-effect loops)
    let current_acc_id = selected_account_id();
    let effective_account_id =
        if current_acc_id != 0 && unique_accounts.iter().any(|(id, _)| *id == current_acc_id) {
            current_acc_id
        } else if !trimmed_filter.is_empty()
            && unique_accounts.len() < AUTO_SELECT_THRESHOLD
            && !unique_accounts.is_empty()
        {
            unique_accounts[0].0
        } else {
            0
        };

    let selectable_emails: Vec<_> = candidates
        .iter()
        .filter(|c| c.account_id == effective_account_id)
        .collect();

    // Determine effective email id purely from state
    let current_email_id = selected_email_id();
    let effective_email_id = if current_email_id != 0
        && selectable_emails
            .iter()
            .any(|c| c.account_email_id == current_email_id)
    {
        current_email_id
    } else {
        selectable_emails
            .first()
            .map(|c| c.account_email_id)
            .unwrap_or(0)
    };

    let status_options = if is_admin {
        ALL_STATUSES
    } else {
        MODERATOR_STATUSES
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
                                        let _ = update_config(None, None, None, false, Some(0), None, None, true, None, false, None, None, None, false);
                                    } else {
                                        let _ = update_config(None, None, None, false, Some(0), None, Some(trimmed), false, None, false, None, None, None, false);
                                    }
                                }
                            },
                        }
                    }
                    if !is_admin && !trimmed_filter.is_empty() && trimmed_filter.chars().count() < 5 {
                        FormText { class: "text-muted mb-2", {tid!("subscriber-search-min-chars")} }
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
                            if unique_accounts.is_empty() {
                                {tid!("general-no-results")}
                            } else {
                                {tid!("subscriber-select-member")}
                            }
                        }
                        for (acc_id, acc_name) in unique_accounts.clone() {
                            option {
                                key: "{acc_id}",
                                value: "{acc_id}",
                                "{acc_name}"
                            }
                        }
                    }
                    if !trimmed_filter.is_empty() && (is_admin || trimmed_filter.chars().count() >= 5) {
                        FormText {
                            "{filter_count_text}"
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
                                    key: "{email.account_email_id}",
                                    value: "{email.account_email_id}",
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
                        for s in status_options {
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
                            let _ = update_config(None, None, None, false, Some(0), None, None, true, None, false, None, None, None, false);
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
                                    let _ = update_config(None, None, None, false, Some(0), None, None, true, None, false, None, None, None, false);
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
    let is_admin = use_is_admin();

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
    let is_target_moderator = target_val
        .as_ref()
        .map(|t| t.permission == SubscriptionPermission::Moderate)
        .unwrap_or(false);
    let is_target_required = target_val
        .as_ref()
        .map(|t| t.status == SubscriptionStatus::RequiredSubscribed)
        .unwrap_or(false);

    let edit_status_options = if is_admin {
        ALL_STATUSES
    } else {
        MODERATOR_STATUSES
    };

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
                        disabled: !is_admin && (is_target_required || is_target_moderator),
                        onchange: move |e: FormEvent| {
                            if let Some(s) = parse_status(&e.value()) {
                                edit_status.set(s);
                            }
                        },
                        for s in edit_status_options {
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
                            SubscriptionPermission::Moderate => "moderate",
                        },
                        disabled: !is_admin && is_target_moderator,
                        onchange: move |e: FormEvent| {
                            match e.value().as_str() {
                                "read" => edit_permission.set(SubscriptionPermission::Read),
                                "write" => edit_permission.set(SubscriptionPermission::Write),
                                "moderate" if is_admin => edit_permission.set(SubscriptionPermission::Moderate),
                                _ => {}
                            }
                        },
                        option { value: "read", {tid!("subscriber-permission-read")} }
                        option { value: "write", {tid!("subscriber-permission-write")} }
                        if is_admin || edit_permission() == SubscriptionPermission::Moderate {
                            option { value: "moderate", {tid!("subscriber-permission-moderate")} }
                        }
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
                    disabled: !is_admin && is_target_moderator,
                    onclick: move |_| {
                        let Some(t) = target() else { return };
                        let status = edit_status();
                        let permission = edit_permission();
                        info!(
                            "Admin updating subscription: account={}, topic={topic_id}, status={status:?}, permission={permission:?}",
                            t.account_id
                        );

                        let res1 = if status != t.status {
                            admin_add_subscription(t.account_id, t.account_email_id, topic_id, status)
                        } else {
                            Ok(())
                        };
                        let res2 = if permission != t.permission {
                            update_permission(t.subscription_id, permission)
                        } else {
                            Ok(())
                        };

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
