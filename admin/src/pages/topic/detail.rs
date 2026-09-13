use std::collections::HashSet;

use ::dioxus::{logger::tracing::error, prelude::*};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::dioxus::{
    use_reducer_clear_topic_provisioning_lock, use_reducer_update_message_topic,
    use_subscription, use_table_visible_account_emails, use_table_visible_accounts,
    use_table_visible_categories, use_table_visible_message_topic_categories,
    use_table_visible_message_topics, use_table_visible_subscriptions,
};
use crate::module_bindings::{MessageTopic, SubscriptionPermission, TopicVisibility};
use crate::pages::topic::categories::TopicCategoriesCard;
use crate::pages::topic::modals::{
    AddSubscriberModal, EditSubscriptionModal, EditSubscriptionTarget,
};
use crate::pages::topic::subscribers::TopicSubscribersCard;

/// Form card for editing a topic's name, description, and visibility.
#[component]
pub fn TopicDetailsCard(
    topic: MessageTopic,
    mut name: Signal<String>,
    mut description: Signal<String>,
    mut visibility: Signal<TopicVisibility>,
    mut default_permission: Signal<SubscriptionPermission>,
    mut save_message: Signal<Option<(String, Color)>>,
) -> Element {
    let update_topic = use_reducer_update_message_topic();
    let clear_lock = use_reducer_clear_topic_provisioning_lock();
    let topic_id = topic.id;
    let visibility_value = match visibility() {
        TopicVisibility::Public => "Public",
        TopicVisibility::Private => "Private",
    };
    let default_permission_value = match default_permission() {
        SubscriptionPermission::Read => "Read",
        SubscriptionPermission::Write => "Write",
    };

    rsx! {
        Card {
            class: "shadow-sm h-100",
            header_class: "bg-primary text-white",
            header: rsx! {
                h5 { class: "card-title mb-0",
                    Icon { name: "pencil-square", class: "me-2" }
                    {tid!("topic-detail-title")}
                }
            },
            body: rsx! {
                if topic.locked_is_provisioning {
                    Alert {
                        color: Color::Warning,
                        class: "d-flex justify-content-between align-items-center mb-3",
                        div {
                            Icon { name: "lock-fill", class: "me-2" }
                            {tid!("topic-provisioning-locked-warning")}
                        }
                        Button {
                            color: Color::Warning,
                            size: Size::Sm,
                            onclick: move |_| {
                                match clear_lock(topic_id) {
                                    Ok(()) => {
                                        save_message.set(Some((tid!("topic-clear-provisioning-lock-success"), Color::Success)));
                                    }
                                    Err(e) => {
                                        error!("clear_topic_provisioning_lock failed: {e:?}");
                                        save_message.set(Some((tid!("topic-detail-save-error", error: format!("{e:?}")), Color::Danger)));
                                    }
                                }
                            },
                            Icon { name: "unlock-fill", class: "me-1" }
                            {tid!("topic-clear-provisioning-lock")}
                        }
                    }
                }
                if let Some((msg, color)) = save_message.read().clone() {
                    Alert { color, class: "mb-3", "{msg}" }
                }
                FormGroup { label: tid!("topic-detail-name"),
                    Input {
                        r#type: "text",
                        value: "{name}",
                        oninput: move |e: FormEvent| name.set(e.value()),
                    }
                }
                FormGroup { label: tid!("topic-detail-description"),
                    Textarea {
                        rows: 3,
                        value: "{description}",
                        oninput: move |e: FormEvent| description.set(e.value()),
                    }
                }
                FormGroup { label: tid!("topic-detail-visibility"),
                    Select {
                        value: visibility_value,
                        onchange: move |e: FormEvent| {
                            match e.value().as_str() {
                                "Public" => visibility.set(TopicVisibility::Public),
                                "Private" => visibility.set(TopicVisibility::Private),
                                _ => {}
                            }
                        },
                        option { value: "Public", {tid!("topic-visibility-public")} }
                        option { value: "Private", {tid!("topic-visibility-private")} }
                    }
                    FormText {
                        {tid!("topic-detail-visibility-help")}
                    }
                }
                FormGroup { label: tid!("topic-detail-default-permission"),
                    Select {
                        value: default_permission_value,
                        onchange: move |e: FormEvent| {
                            match e.value().as_str() {
                                "Read" => default_permission.set(SubscriptionPermission::Read),
                                "Write" => default_permission.set(SubscriptionPermission::Write),
                                _ => {}
                            }
                        },
                        option { value: "Read", {tid!("topic-permission-read")} }
                        option { value: "Write", {tid!("topic-permission-write")} }
                    }
                    FormText {
                        {tid!("topic-detail-default-permission-help")}
                    }
                }
                FormGroup { label: tid!("topic-detail-email"),
                    Input {
                        r#type: "text",
                        value: "{topic.email_address}",
                        disabled: true,
                        readonly: true,
                    }
                    FormText { {tid!("topic-detail-email-help")} }
                }
                Button {
                    color: Color::Primary,
                    disabled: name.read().trim().is_empty(),
                    onclick: move |_| {
                        let n = name.read().clone();
                        let d = description.read().clone();
                        let v = visibility.read().clone();
                        let p = default_permission.read().clone();
                        match update_topic(topic_id, n, d, Some(v), Some(p), None) {
                            Ok(()) => {
                                save_message
                                    .set(Some((tid!("topic-detail-saved"), Color::Success)));
                            }
                            Err(e) => {
                                error!("update_message_topic failed: {e:?}");
                                save_message
                                    .set(Some((tid!("topic-detail-save-error", error: format!("{e:?}")), Color::Danger)));
                            }
                        }
                    },
                    Icon { name: "check-lg", class: "me-2" }
                    {tid!("topic-detail-save")}
                }
            },
        }
    }
}

/// Admin-only detail/edit view for a single message topic (mailing list).
#[component]
pub fn TopicDetailPage(topic_id: u64, on_back: EventHandler<()>) -> Element {
    use_subscription(&[
        "SELECT * FROM visible_accounts",
        "SELECT * FROM visible_account_emails",
        "SELECT * FROM visible_message_topics",
        "SELECT * FROM visible_subscriptions",
        "SELECT * FROM visible_categories",
        "SELECT * FROM visible_message_topic_categories",
        "SELECT * FROM topic_subscriber_counts",
        "SELECT * FROM visible_account_configs",
        "SELECT * FROM total_accounts",
    ]);
    let topics = use_table_visible_message_topics();
    let subscriptions = use_table_visible_subscriptions();
    let accounts = use_table_visible_accounts();
    let account_emails = use_table_visible_account_emails();
    let categories = use_table_visible_categories();
    let topic_categories = use_table_visible_message_topic_categories();

    let topic = use_memo(move || topics().into_iter().find(|t| t.id == topic_id));

    // Local edit state, seeded once from the loaded topic.
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut visibility = use_signal(|| TopicVisibility::Public);
    let mut default_permission = use_signal(|| SubscriptionPermission::Read);
    let mut initialized = use_signal(|| false);
    let save_message: Signal<Option<(String, Color)>> = use_signal(|| None);
    let categories_message: Signal<Option<(String, Color)>> = use_signal(|| None);
    let new_category_name = use_signal(String::new);
    let renaming_category_id: Signal<Option<u64>> = use_signal(|| None);
    let rename_draft = use_signal(String::new);

    use_effect(move || {
        if let Some(t) = topic() {
            if !initialized() {
                name.set(t.name.clone());
                description.set(t.description.clone());
                visibility.set(t.visibility);
                default_permission.set(t.default_permission);
                initialized.set(true);
            }
        }
    });

    // Modal display states
    let show_add_modal = use_signal(|| false);
    let show_edit_modal = use_signal(|| false);
    let edit_target: Signal<Option<EditSubscriptionTarget>> = use_signal(|| None);

    let Some(t) = topic() else {
        return rsx! {
            Container { fluid: true, class: "mt-4",
                Alert { color: Color::Warning, class: "d-flex align-items-center",
                    Icon { name: "exclamation-triangle", class: "me-2" }
                    {tid!("topic-detail-not-found")}
                }
                Button {
                    color: Color::Secondary,
                    onclick: move |_| on_back.call(()),
                    Icon { name: "arrow-left", class: "me-2" }
                    {tid!("topic-detail-back")}
                }
            }
        };
    };

    let topic_subscriptions: Vec<_> = subscriptions()
        .into_iter()
        .filter(|s| s.topic_id == topic_id)
        .collect();

    let topic_subscribed_email_ids: HashSet<u64> = topic_subscriptions
        .iter()
        .filter(|s| crate::pages::is_active_subscription(&s.status))
        .map(|s| s.account_email_id)
        .collect();

    let all_account_emails = account_emails();
    let available_accounts: Vec<_> = accounts()
        .into_iter()
        .filter(|a| {
            let acct_emails: Vec<_> = all_account_emails
                .iter()
                .filter(|e| e.account_id == a.id)
                .collect();
            acct_emails.iter().any(|e| e.is_verified && !topic_subscribed_email_ids.contains(&e.id))
        })
        .collect();

    let assigned_category_ids: HashSet<u64> = topic_categories()
        .into_iter()
        .filter(|link| link.topic_id == topic_id)
        .map(|link| link.category_id)
        .collect();

    // Stable ids only — names come from the live `categories` signal inside each row.
    let category_ids: Vec<u64> = {
        let mut rows = categories();
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        rows.into_iter().map(|c| c.id).collect()
    };

    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3",
                Col {
                    Button {
                        color: Color::Secondary,
                        size: Size::Sm,
                        class: "mb-2",
                        onclick: move |_| on_back.call(()),
                        Icon { name: "arrow-left", class: "me-2" }
                        {tid!("topic-detail-back")}
                    }
                    h2 { class: "mb-0",
                        Icon { name: "envelope-fill", class: "me-2" }
                        "{t.name}"
                        if t.active {
                            Badge {
                                color: Color::Success,
                                class: "ms-2 align-middle",
                                {tid!("topic-status-active")}
                            }
                        } else {
                            Badge {
                                color: Color::Secondary,
                                class: "ms-2 align-middle",
                                {tid!("topic-status-inactive")}
                            }
                        }
                        if t.visibility == TopicVisibility::Public {
                            Badge {
                                color: Color::Info,
                                class: "ms-2 align-middle",
                                {tid!("topic-visibility-public")}
                            }
                        } else {
                            Badge {
                                color: Color::Warning,
                                class: "ms-2 align-middle",
                                {tid!("topic-visibility-private")}
                            }
                        }
                        if t.locked_is_provisioning {
                            Badge {
                                color: Color::Danger,
                                class: "ms-2 align-middle",
                                {tid!("topic-status-provisioning-locked")}
                            }
                        }
                    }
                }
            }

            Row { class: "mb-4",
                Col { lg: ColumnSize::Span(6), class: "mb-3",
                    TopicDetailsCard {
                        topic: t,
                        name,
                        description,
                        visibility,
                        default_permission,
                        save_message,
                    }
                }
                Col { lg: ColumnSize::Span(6), class: "mb-3",
                    TopicCategoriesCard {
                        topic_id,
                        assigned_category_ids,
                        category_ids,
                        categories_message,
                        new_category_name,
                        renaming_category_id,
                        rename_draft,
                    }
                }
            }

            Row {
                Col { xs: ColumnSize::Span(12),
                    TopicSubscribersCard {
                        topic_id,
                        show_add_modal,
                        show_edit_modal,
                        edit_target,
                    }
                }
            }

            AddSubscriberModal {
                show: show_add_modal,
                topic_id,
                available_accounts,
                available_emails: account_emails(),
                subscribed_email_ids: topic_subscribed_email_ids,
            }

            EditSubscriptionModal { show: show_edit_modal, topic_id, target: edit_target }
        }
    }
}
