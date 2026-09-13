use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::dioxus::{
    use_procedure_provision_message_topic, use_reducer_remove_message_topic,
    use_subscription, use_table_visible_domains, use_table_visible_message_topics,
    use_table_topic_subscriber_counts,
};
use crate::module_bindings::TopicVisibility;
use crate::pages::topic::detail::TopicDetailPage;

/// Card with form controls to create and provision a new message topic / mailing list.
#[component]
pub fn AddTopicCard() -> Element {
    let domains = use_table_visible_domains();
    let (add_invoke, add_result) = use_procedure_provision_message_topic();

    let mut name = use_signal(String::new);
    let mut base = use_signal(String::new);
    let mut selected_domain: Signal<Option<(String, String)>> = use_signal(|| None);
    let mut description = use_signal(String::new);
    let mut visibility = use_signal(|| TopicVisibility::Public);
    let add_error: Signal<Option<(String, Color)>> = use_signal(|| None);
    let is_sending = use_signal(|| false);

    // React to procedure result signal and update UI accordingly.
    {
        let mut add_result = add_result.clone();
        let mut name = name.clone();
        let mut base = base.clone();
        let mut selected_domain = selected_domain.clone();
        let mut description = description.clone();
        let mut visibility = visibility.clone();
        let mut add_error = add_error.clone();
        let mut is_sending = is_sending.clone();

        use_effect(move || {
            if let Some(res) = add_result() {
                is_sending.set(false);
                match res {
                    Ok(inner) => match inner {
                        Ok(()) => {
                            name.set(String::new());
                            base.set(String::new());
                            selected_domain.set(None);
                            description.set(String::new());
                            visibility.set(TopicVisibility::Public);
                            add_error.set(Some((
                                tid!("topic-add-success"),
                                Color::Success,
                            )));
                        }
                        Err(proc_err) => {
                            error!("provision_message_topic failed: {proc_err}");
                            add_error.set(Some((proc_err, Color::Danger)));
                        }
                    },
                    Err(internal_err) => {
                        error!("provision_message_topic internal error: {internal_err}");
                        add_error.set(Some((internal_err, Color::Danger)));
                    }
                }

                // Clear the result so the next invocation can be observed
                add_result.set(None);
            }
        });
    }

    let domain_value = selected_domain
        .read()
        .as_ref()
        .map(|(id, name)| format!("{id}:{name}"))
        .unwrap_or_default();
    let visibility_value = match visibility() {
        TopicVisibility::Public => "Public",
        TopicVisibility::Private => "Private",
    };

    rsx! {
        Card {
            class: "shadow-sm",
            header_class: "bg-primary text-white",
            header: rsx! {
                h5 { class: "card-title mb-0",
                    Icon { name: "plus-circle", class: "me-2" }
                    {tid!("topic-add-title")}
                }
            },
            body: rsx! {
                if add_error().is_some() {
                    Alert {
                        color: add_error.read().clone().unwrap_or_default().1,
                        class: "mb-3 d-flex align-items-start",
                        Icon { name: "exclamation-circle", class: "me-2 mt-1 flex-shrink-0" }
                        "{add_error.read().clone().unwrap_or_default().0}"
                    }
                }
                Row { class: "g-3 align-items-end",
                    Col { md: ColumnSize::Span(3),
                        FormGroup { label: tid!("topic-form-name"),
                            Input {
                                r#type: "text",
                                placeholder: tid!("topic-form-name-placeholder"),
                                value: "{name}",
                                oninput: move |e: FormEvent| name.set(e.value()),
                            }
                        }
                    }
                    Col { md: ColumnSize::Span(4),
                        FormGroup { label: tid!("topic-form-email"),
                            InputGroup {
                                Input {
                                    r#type: "text",
                                    placeholder: tid!("topic-form-mailbox-placeholder"),
                                    value: "{base}",
                                    oninput: move |e: FormEvent| base.set(e.value()),
                                }
                                InputGroupText { "@" }
                                Select {
                                    value: domain_value,
                                    onchange: move |e: FormEvent| {
                                        let val = e.value();
                                        if val.is_empty() {
                                            selected_domain.set(None);
                                        } else if let Some(idx) = val.find(':') {
                                            let id = val[..idx].to_string();
                                            let dname = val[idx + 1..].to_string();
                                            selected_domain.set(Some((id, dname)));
                                        }
                                    },
                                    option { value: "", {tid!("topic-form-domain-select")} }
                                    for domain in domains() {
                                        option {
                                            key: "{domain.id}",
                                            value: "{domain.id}:{domain.name}",
                                            "{domain.name}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Col { md: ColumnSize::Span(3),
                        FormGroup { label: tid!("topic-form-description"),
                            Input {
                                r#type: "text",
                                placeholder: tid!("topic-form-description-placeholder"),
                                value: "{description}",
                                oninput: move |e: FormEvent| description.set(e.value()),
                            }
                        }
                    }
                    Col { md: ColumnSize::Span(1),
                        FormGroup { label: tid!("topic-form-visibility"),
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
                        }
                    }
                    Col { md: ColumnSize::Span(1),
                        FormGroup { label: " ",
                            Button {
                                color: Color::Primary,
                                class: "w-100",
                                disabled: name.read().is_empty()
                                    || base.read().is_empty()
                                    || selected_domain.read().is_none()
                                    || *is_sending.read(),
                                onclick: {
                                    let add = add_invoke.clone();
                                    let mut is_sending = is_sending.clone();
                                    let visibility_signal = visibility.clone();
                                    move |_| {
                                        let n = name.read().clone();
                                        let b = base.read().clone();
                                        let (domain_id, _) = selected_domain.read().clone().unwrap_or_default();
                                        let d = description.read().clone();
                                        let v = visibility_signal.read().clone();
                                        is_sending.set(true);
                                        add(n, b, domain_id, d, v);
                                    }
                                },
                                Icon { name: "plus-lg" }
                            }
                        }
                    }
                }
            },
        }
    }
}

/// Table displaying all existing message topics.
#[component]
pub fn TopicTable(mut selected_topic: Signal<Option<u64>>) -> Element {
    let topics = use_table_visible_message_topics();
    let subscriber_counts = use_table_topic_subscriber_counts();
    let remove_topic = use_reducer_remove_message_topic();

    rsx! {
        Card {
            class: "shadow-sm",
            header_class: "bg-primary text-white",
            body_class: "p-0",
            header: rsx! {
                h5 { class: "card-title mb-0",
                    Icon { name: "list-ul", class: "me-2" }
                    {tid!("topic-table-title")}
                    span { class: "badge bg-white text-primary ms-2", "{topics().len()}" }
                }
            },
            body: rsx! {
                if topics().is_empty() {
                    div { class: "p-4 text-muted",
                        Icon { name: "inbox", class: "me-2" }
                        {tid!("topic-table-empty")}
                    }
                } else {
                    Table { hover: true, responsive: true, class: "mb-0",
                        thead { class: "table-light",
                            tr {
                                th { {tid!("topic-table-th-name")} }
                                th { {tid!("topic-table-th-email")} }
                                th { {tid!("topic-table-th-description")} }
                                th { {tid!("topic-table-th-status")} }
                                th { {tid!("topic-table-th-visibility")} }
                                th { class: "text-end", {tid!("topic-table-th-subscribers")} }
                                th { class: "text-end", {tid!("topic-table-th-actions")} }
                            }
                        }
                        tbody {
                            for top in topics() {
                                {
                                    let top_id = top.id;
                                    let remove = remove_topic.clone();
                                    let subscriber_count = subscriber_counts()
                                        .iter()
                                        .find(|c| c.topic_id == top_id)
                                        .map(|c| c.count)
                                        .unwrap_or(0);
                                    rsx! {
                                        tr {
                                            key: "{top_id}",
                                            style: "cursor: pointer;",
                                            onclick: move |_| selected_topic.set(Some(top_id)),
                                            td {
                                                strong { "{top.name}" }
                                            }
                                            td {
                                                code { "{top.email_address}" }
                                            }
                                            td { class: "text-muted", "{top.description}" }
                                            td {
                                                if top.active {
                                                    Badge { color: Color::Success, {tid!("topic-status-active")} }
                                                } else {
                                                    Badge { color: Color::Secondary, {tid!("topic-status-inactive")} }
                                                }
                                            }
                                            td {
                                                if top.visibility == TopicVisibility::Public {
                                                    Badge { color: Color::Info, {tid!("topic-visibility-public")} }
                                                } else {
                                                    Badge { color: Color::Warning, {tid!("topic-visibility-private")} }
                                                }
                                            }
                                            td { class: "text-end",
                                                Badge { color: Color::Primary, "{subscriber_count}" }
                                            }
                                            td { class: "text-end",
                                                Button {
                                                    color: Color::Primary,
                                                    size: Size::Sm,
                                                    class: "me-1",
                                                    onclick: move |evt: MouseEvent| {
                                                        evt.stop_propagation();
                                                        selected_topic.set(Some(top_id));
                                                    },
                                                    Icon { name: "pencil-square", class: "me-1" }
                                                    {tid!("topic-action-details")}
                                                }
                                                Button {
                                                    color: Color::Danger,
                                                    size: Size::Sm,
                                                    onclick: move |evt: MouseEvent| {
                                                        evt.stop_propagation();
                                                        info!("Removing topic {top_id}");
                                                        if let Err(e) = remove(top_id) {
                                                            error!("remove_message_topic failed: {e:?}");
                                                        }
                                                    },
                                                    Icon { name: "trash", class: "me-1" }
                                                    {tid!("topic-action-delete")}
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

/// Admin-only view: lists all message topics with inline add and delete controls.
#[component]
pub fn TopicsPage() -> Element {
    use_subscription(&[
        "SELECT * FROM visible_message_topics",
        "SELECT * FROM visible_subscriptions",
        "SELECT * FROM visible_domains",
        "SELECT * FROM topic_subscriber_counts",
    ]);

    // When set, the detail/edit page for this topic is shown instead of the list.
    let mut selected_topic: Signal<Option<u64>> = use_signal(|| None);

    rsx! {
        if let Some(id) = selected_topic() {
            TopicDetailPage { topic_id: id, on_back: move |_| selected_topic.set(None) }
        } else {
            Container { fluid: true, class: "mt-4",
                Row { class: "mb-3",
                    Col {
                        h2 { class: "mb-0",
                            Icon { name: "tags-fill", class: "me-2" }
                            {tid!("topic-page-title")}
                        }
                    }
                }

                Row { class: "mb-4",
                    Col { xs: ColumnSize::Span(12),
                        AddTopicCard {}
                    }
                }

                Row {
                    Col { xs: ColumnSize::Span(12),
                        TopicTable { selected_topic }
                    }
                }
            }
        }
    }
}
