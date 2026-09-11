use std::collections::HashSet;

use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::SubscriptionStatus;
use crate::module_bindings::{
    dioxus::{
        use_reducer_add_subscription, use_reducer_remove_account_email,
        use_reducer_remove_subscription, use_reducer_user_request_email_verification,
        use_subscription, use_table_visible_message_categories,
        use_table_visible_message_category_topics, use_table_visible_subscriptions,
        use_table_visible_topics,
    },
    CategoryVisibility, EmailSource, MessageCategory,
};
use crate::oauth::UserInfo;

/// Tab identity for the member subscriptions page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TopicTab {
    Topic(u64),
    Sonstige,
}

/// Default view for all users: lists all active message categories and lets the
/// user subscribe or unsubscribe with a single button click. Categories are
/// grouped into tabs by topic; categories without topics appear under "Sonstige".
#[component]
pub fn SubscriptionsPage(user_info: UserInfo) -> Element {
    use_subscription(&[
        "SELECT * FROM visible_accounts",
        "SELECT * FROM visible_account_emails",
        "SELECT * FROM visible_message_categories",
        "SELECT * FROM visible_subscriptions",
        "SELECT * FROM visible_topics",
        "SELECT * FROM visible_message_category_topics",
    ]);
    let accounts = crate::module_bindings::dioxus::use_table_visible_accounts();
    let account_emails = crate::module_bindings::dioxus::use_table_visible_account_emails();
    let categories = use_table_visible_message_categories();
    let subscriptions = use_table_visible_subscriptions();
    let topics = use_table_visible_topics();
    let category_topics = use_table_visible_message_category_topics();
    let add_subscription = use_reducer_add_subscription();
    let remove_subscription = use_reducer_remove_subscription();

    let add_email = use_reducer_user_request_email_verification();
    let remove_email = use_reducer_remove_account_email();
    let mut show_add_email = use_signal(|| false);
    let mut add_email_input = use_signal(|| String::new());

    let account_id: u64 = user_info.mitgliedsnr.parse().unwrap_or(0);
    let my_account = accounts().into_iter().find(|a| a.id == account_id);
    let my_primary_email_id = my_account.map(|a| a.primary_email_id).unwrap_or(0);
    let my_emails: Vec<_> = account_emails()
        .into_iter()
        .filter(|e| e.account_id == account_id)
        .collect();

    let mut active_tab = use_signal(|| TopicTab::Sonstige);
    let mut user_picked_tab = use_signal(|| false);

    let active_cats: Vec<MessageCategory> = categories().into_iter().filter(|c| c.active).collect();

    let topic_ids: Vec<u64> = {
        let mut rows = topics();
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        rows.into_iter().map(|t| t.id).collect()
    };

    let links = category_topics();
    let categorized_ids: HashSet<u64> = links.iter().map(|l| l.category_id).collect();

    let sonstige_cats: Vec<MessageCategory> = active_cats
        .iter()
        .filter(|c| !categorized_ids.contains(&c.id))
        .cloned()
        .collect();

    let show_sonstige = !sonstige_cats.is_empty() || topic_ids.is_empty();

    // Keep the default tab in sync with loaded data until the user picks one.
    use_effect(move || {
        if user_picked_tab() {
            return;
        }
        let mut topics_sorted = topics();
        topics_sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        if let Some(first) = topics_sorted.first() {
            active_tab.set(TopicTab::Topic(first.id));
        } else {
            active_tab.set(TopicTab::Sonstige);
        }
    });

    let current = active_tab();
    let visible_cats: Vec<MessageCategory> = match current {
        TopicTab::Sonstige => sonstige_cats.clone(),
        TopicTab::Topic(topic_id) => {
            let cat_ids: HashSet<u64> = links
                .iter()
                .filter(|l| l.topic_id == topic_id)
                .map(|l| l.category_id)
                .collect();
            active_cats
                .into_iter()
                .filter(|c| cat_ids.contains(&c.id))
                .collect()
        }
    };

    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "envelope-check", class: "me-2" }
                        "{tid!(\"subscriptions-page-title\") }"
                    }
                    p { class: "text-muted mt-1",
                        "{tid!(\"subscriptions-page-description\") }"
                    }
                }
            }

            Card { class: "mb-4 shadow-sm",
                header: rsx! {
                    h5 { class: "mb-0", "{tid!(\"subscriptions-linked-emails-title\")}" }
                },
                body: rsx! {
                    ul { class: "list-group list-group-flush mb-3",
                        for email in &my_emails {
                            {
                                let email_id = email.id;
                                let remove_email_for_row = remove_email.clone();
                                rsx! {
                                    li { class: "list-group-item d-flex justify-content-between align-items-center px-0",
                                        div {
                                            if email.id == my_primary_email_id {
                                                strong { "{email.email} " }
                                                span { class: "text-muted", "{tid!(\"subscriptions-primary-badge\")}" }
                                            } else {
                                                span { "{email.email}" }
                                            }
                                            if !email.is_verified {
                                                Badge { color: Color::Warning, class: "ms-2", "{tid!(\"subscriptions-email-pending-verification\")}" }
                                            }
                                        }
                                        if email.source != EmailSource::DjangoSync && email.id != my_primary_email_id {
                                            Button {
                                                color: Color::Danger,
                                                size: Size::Sm,
                                                onclick: move |_| {
                                                    if let Err(e) = remove_email_for_row(email_id) {
                                                        error!("remove_account_email failed: {e:?}");
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
                                                    error!("Failed to request email verification: {e:?}");
                                                } else {
                                                    show_add_email.set(false);
                                                    add_email_input.set(String::new());
                                                }
                                            }
                                        },
                                        "{tid!(\"subscriptions-send-verification\")}"
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
                                "{tid!(\"general-cancel\")}"
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
                            "{tid!(\"subscriptions-add-email-button\")}"
                        }
                    }
                },
            }

            if !topic_ids.is_empty() || show_sonstige {
                Nav {
                    tabs: true,
                    class: "mb-3",
                    for topic_id in topic_ids.iter().copied() {
                        TopicTabButton {
                            key: "{topic_id}",
                            topic_id,
                            active: current == TopicTab::Topic(topic_id),
                            on_select: move |_| {
                                user_picked_tab.set(true);
                                active_tab.set(TopicTab::Topic(topic_id));
                            },
                        }
                    }
                    if show_sonstige {
                        NavItem {
                            NavLink {
                                active: current == TopicTab::Sonstige,
                                prevent_default: true,
                                onclick: move |_| {
                                    user_picked_tab.set(true);
                                    active_tab.set(TopicTab::Sonstige);
                                },
                                "{tid!(\"subscriptions-tab-other\") }"
                            }
                        }
                    }
                }
            }

            if visible_cats.is_empty() {
                Alert { color: Color::Info,
                    Icon { name: "info-circle", class: "me-2" }
                    if topic_ids.is_empty() && sonstige_cats.is_empty() {
                        "{tid!(\"subscriptions-empty\") }"
                    } else {
                        "{tid!(\"subscriptions-empty-category\") }"
                    }
                }
            } else {
                Row {
                    for cat in visible_cats {
                        {
                            let cat_subscriptions: Vec<_> = subscriptions().into_iter().filter(|s| {
                                s.category_id == cat.id
                                    && s.subscriber_account_id == account_id
                                    && crate::pages::is_active_subscription(&s.status)
                            }).collect();
                            let is_subscribed_any = !cat_subscriptions.is_empty();
                            let cat_id = cat.id;
                            let add = add_subscription.clone();
                            let remove = remove_subscription.clone();
                            let my_emails_clone = my_emails.clone();

                            rsx! {
                                Col { md: ColumnSize::Span(6), lg: ColumnSize::Span(4), class: "mb-3",
                                    Card {
                                        class: if is_subscribed_any { "h-100 border-dark bg-light" } else { "h-100 border-light" },
                                        body_class: "d-flex flex-column",
                                        header: rsx! {
                                            h5 { class: "card-title mb-0", "{cat.name}" }
                                            if is_subscribed_any {
                                                Badge { color: Color::Success, class: "ms-2", "{tid!(\"subscriptions-subscribed\")}" }
                                            }

                                            if cat.visibility == CategoryVisibility::Public {
                                                Badge { color: Color::Info, class: "ms-2 align-middle", "{tid!(\"subscriptions-public\")}" }
                                            } else {
                                                Badge { color: Color::Warning, class: "ms-2 align-middle", "{tid!(\"subscriptions-private\")}" }
                                            }
                                        },
                                        body: rsx! {
                                            p { class: "card-text text-muted small flex-grow-1", "{cat.description}" }
                                            p { class: "card-text mb-3",
                                                small { class: "text-muted",
                                                    Icon { name: "envelope", class: "me-1" }
                                                    "{cat.email_address}"
                                                }
                                            }
                                            div { class: "mt-auto pt-2 border-top",
                                                small { class: "text-muted fw-bold d-block mb-2", "{tid!(\"subscriptions-card-email-subscriptions\")}" }
                                                div { class: "d-flex flex-column gap-2",
                                                    for email in my_emails_clone {
                                                        {
                                                            let email_id = email.id;
                                                            let sub_for_email = cat_subscriptions.iter().find(|s| s.account_email_id == email_id);
                                                            let is_subbed = sub_for_email.is_some();
                                                            let is_required = sub_for_email.is_some_and(|s| matches!(s.status, SubscriptionStatus::RequiredSubscribed));
                                                            let sub_id = sub_for_email.map(|s| s.id);
                                                            let add_fn = add.clone();
                                                            let remove_fn = remove.clone();
                                                            let input_id = format!("sub-check-{cat_id}-{email_id}");

                                                            rsx! {
                                                                div { class: "form-check",
                                                                    input {
                                                                        class: "form-check-input",
                                                                        r#type: "checkbox",
                                                                        id: "{input_id}",
                                                                        checked: is_subbed,
                                                                        disabled: is_required || !email.is_verified,
                                                                        onchange: move |_| {
                                                                            if is_subbed {
                                                                                if let Some(id) = sub_id {
                                                                                    info!("Unsubscribing {email_id} from category {cat_id}");
                                                                                    if let Err(err) = remove_fn(id) {
                                                                                        error!("remove_subscription failed: {err:?}");
                                                                                    }
                                                                                }
                                                                            } else {
                                                                                info!("Subscribing {email_id} to category {cat_id}");
                                                                                if let Err(err) = add_fn(account_id, email_id, cat_id) {
                                                                                    error!("add_subscription failed: {err:?}");
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    label {
                                                                        class: "form-check-label small d-flex align-items-center flex-wrap gap-1",
                                                                        r#for: "{input_id}",
                                                                        if email.id == my_primary_email_id {
                                                                            strong { "{email.email} " }
                                                                            span { class: "text-muted", "{tid!(\"subscriptions-primary-badge\")}" }
                                                                        } else {
                                                                            span { "{email.email}" }
                                                                        }
                                                                        if !email.is_verified {
                                                                            Badge { color: Color::Secondary, class: "ms-1", "{tid!(\"subscriptions-email-unverified\")}" }
                                                                        }
                                                                        if is_required {
                                                                            Badge { color: Color::Info, class: "ms-1", "{tid!(\"subscriptions-required\")}" }
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
                }
            }
        }
    }
}

/// Tab button that always reads the topic name from the live `visible_topics` signal.
#[component]
fn TopicTabButton(topic_id: u64, active: bool, on_select: EventHandler<()>) -> Element {
    let topics = use_table_visible_topics();
    let name = use_memo(move || {
        topics()
            .into_iter()
            .find(|t| t.id == topic_id)
            .map(|t| t.name)
            .unwrap_or_default()
    });
    let label = name();
    if label.is_empty() {
        return rsx! {};
    }

    rsx! {
        NavItem {
            NavLink {
                active,
                prevent_default: true,
                onclick: move |_| on_select.call(()),
                "{label}"
            }
        }
    }
}
