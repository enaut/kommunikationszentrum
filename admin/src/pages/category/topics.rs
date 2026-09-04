use std::collections::HashSet;

use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::dioxus::{
    use_reducer_rename_topic, use_reducer_set_category_topics,
    use_table_visible_message_category_topics, use_table_visible_topics,
};
use crate::module_bindings::{MessageCategoryTopic, Topic};

/// Live topic names currently assigned to `category_id`.
pub fn current_assigned_topic_names(
    category_id: u64,
    topics: &[Topic],
    links: &[MessageCategoryTopic],
) -> Vec<String> {
    let assigned: HashSet<u64> = links
        .iter()
        .filter(|link| link.category_id == category_id)
        .map(|link| link.topic_id)
        .collect();
    topics
        .iter()
        .filter(|t| assigned.contains(&t.id))
        .map(|t| t.name.clone())
        .collect()
}

/// One topic row: reads the current name from the `visible_topics` signal so renames
/// show up immediately without relying on a frozen clone from the parent.
#[component]
pub fn TopicCheckRow(
    category_id: u64,
    topic_id: u64,
    is_checked: bool,
    mut renaming_topic_id: Signal<Option<u64>>,
    mut rename_draft: Signal<String>,
    mut topics_message: Signal<Option<(String, Color)>>,
) -> Element {
    let topics = use_table_visible_topics();
    let category_topics = use_table_visible_message_category_topics();
    let set_category_topics = use_reducer_set_category_topics();
    let rename_topic_key = use_reducer_rename_topic();
    let rename_topic_blur = rename_topic_key.clone();

    let topic_name = use_memo(move || {
        topics()
            .into_iter()
            .find(|t| t.id == topic_id)
            .map(|t| t.name)
            .unwrap_or_default()
    });

    let name = topic_name();
    if name.is_empty() && topics().iter().all(|t| t.id != topic_id) {
        return rsx! {};
    }

    let is_renaming = renaming_topic_id() == Some(topic_id);

    rsx! {
        ListGroupItem {
            tag: "div",
            class: "d-flex align-items-center gap-2 position-relative",
            if is_renaming {
                input {
                    class: "form-control",
                    r#type: "text",
                    value: "{rename_draft}",
                    autofocus: true,
                    onmounted: move |evt| {
                        let _ = evt.data().set_focus(true);
                    },
                    oninput: move |e| rename_draft.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Escape {
                            renaming_topic_id.set(None);
                            return;
                        }
                        if e.key() != Key::Enter {
                            return;
                        }
                        let new_name = rename_draft.read().trim().to_string();
                        if new_name.is_empty() {
                            topics_message
                                .set(Some((tid!("category-topic-name-empty"), Color::Danger)));
                            return;
                        }
                        if new_name == topic_name() {
                            renaming_topic_id.set(None);
                            return;
                        }
                        match rename_topic_key(topic_id, new_name) {
                            Ok(()) => {
                                renaming_topic_id.set(None);
                                topics_message
                                    .set(Some((tid!("category-topic-renamed"), Color::Success)));
                            }
                            Err(e) => {
                                error!("rename_topic failed: {e:?}");
                                topics_message.set(Some((format!("{}: {e:?}", tid!("category-topic-error")), Color::Danger)));
                            }
                        }
                    },
                    onblur: move |_| {
                        if renaming_topic_id() != Some(topic_id) {
                            return;
                        }
                        let new_name = rename_draft.read().trim().to_string();
                        if new_name.is_empty() || new_name == topic_name() {
                            renaming_topic_id.set(None);
                            return;
                        }
                        match rename_topic_blur(topic_id, new_name) {
                            Ok(()) => {
                                renaming_topic_id.set(None);
                                topics_message
                                    .set(Some((tid!("category-topic-renamed"), Color::Success)));
                            }
                            Err(e) => {
                                error!("rename_topic failed: {e:?}");
                                topics_message.set(Some((format!("{}: {e:?}", tid!("category-topic-error")), Color::Danger)));
                            }
                        }
                    },
                }
            } else {
                Checkbox {
                    input_id: "topic-check-{topic_id}",
                    class: "mb-0 flex-grow-1",
                    checked: is_checked,
                    label: name.clone(),
                    onchange: move |_| {
                        let mut next = current_assigned_topic_names(
                            category_id,
                            &topics(),
                            &category_topics(),
                        );
                        let current_name = topic_name();
                        if is_checked {
                            next.retain(|n| n != &current_name);
                        } else if !current_name.is_empty() {
                            next.push(current_name);
                        }
                        next.sort();
                        next.dedup();
                        info!("Setting topics for category {category_id}: {next:?}");
                        match set_category_topics(category_id, next) {
                            Ok(()) => topics_message.set(None),
                            Err(e) => {
                                error!("set_category_topics failed: {e:?}");
                                topics_message.set(Some((format!("{}: {e:?}", tid!("category-topic-error")), Color::Danger)));
                            }
                        }
                    },
                }
                Button {
                    color: Color::Success,
                    class: "position-relative",
                    onclick: move |evt: MouseEvent| {
                        evt.prevent_default();
                        evt.stop_propagation();
                        rename_draft.set(topic_name());
                        renaming_topic_id.set(Some(topic_id));
                    },
                    Icon { name: "pencil" }
                }
            }
        }
    }
}

/// Card managing topic assignments and adding new topic tags.
#[component]
pub fn CategoryTopicsCard(
    category_id: u64,
    assigned_topic_ids: HashSet<u64>,
    topic_ids: Vec<u64>,
    mut topics_message: Signal<Option<(String, Color)>>,
    mut new_topic_name: Signal<String>,
    renaming_topic_id: Signal<Option<u64>>,
    rename_draft: Signal<String>,
) -> Element {
    let topics = use_table_visible_topics();
    let category_topics = use_table_visible_message_category_topics();
    let set_category_topics = use_reducer_set_category_topics();

    rsx! {
        Card {
            class: "shadow-sm h-100",
            header_class: "bg-primary text-white",
            header: rsx! {
                h5 { class: "card-title mb-0",
                    Icon { name: "bookmark-star", class: "me-2" }
                    "{tid!(\"category-topics-title\")}"
                }
            },
            body: rsx! {
                if let Some((msg, color)) = topics_message.read().clone() {
                    Alert { color, class: "mb-3", "{msg}" }
                }
                p { class: "text-muted small mb-3",
                    "{tid!(\"category-topics-description\")}"
                }
                if topic_ids.is_empty() {
                    div { class: "text-muted mb-3",
                        Icon { name: "inbox", class: "me-2" }
                        "{tid!(\"category-topics-empty\")}"
                    }
                } else {
                    ListGroup { tag: "div", class: "mb-3",
                        for topic_id in topic_ids.iter().copied() {
                            TopicCheckRow {
                                key: "{topic_id}",
                                category_id,
                                topic_id,
                                is_checked: assigned_topic_ids.contains(&topic_id),
                                renaming_topic_id,
                                rename_draft,
                                topics_message,
                            }
                        }
                    }
                }
                InputGroup {
                    {
                        let set_topics_enter = set_category_topics.clone();
                        let set_topics_click = set_category_topics.clone();
                        rsx! {
                            Input {
                                r#type: "text",
                                placeholder: tid!("category-topics-placeholder"),
                                value: "{new_topic_name}",
                                oninput: move |e: FormEvent| new_topic_name.set(e.value()),
                                onkeydown: move |e: KeyboardEvent| {
                                    if e.key() == Key::Enter {
                                        let name = new_topic_name.read().trim().to_string();
                                        if name.is_empty() {
                                            return;
                                        }
                                        let mut next = current_assigned_topic_names(
                                             category_id,
                                             &topics(),
                                             &category_topics(),
                                        );
                                        next.push(name);
                                        next.sort();
                                        next.dedup();
                                        match set_topics_enter(category_id, next) {
                                            Ok(()) => {
                                                new_topic_name.set(String::new());
                                                topics_message
                                                    .set(
                                                        Some((
                                                            tid!("category-topic-added"),
                                                            Color::Success,
                                                        )),
                                                    );
                                            }
                                            Err(e) => {
                                                error!("set_category_topics (add) failed: {e:?}");
                                                topics_message.set(Some((format!("{}: {e:?}", tid!("category-topic-error")), Color::Danger)));
                                            }
                                        }
                                    }
                                },
                            }
                            Button {
                                color: Color::Success,
                                disabled: new_topic_name.read().trim().is_empty(),
                                onclick: move |_| {
                                    let name = new_topic_name.read().trim().to_string();
                                    if name.is_empty() {
                                        return;
                                    }
                                    let mut next = current_assigned_topic_names(
                                        category_id,
                                        &topics(),
                                        &category_topics(),
                                    );
                                    next.push(name);
                                    next.sort();
                                    next.dedup();
                                    match set_topics_click(category_id, next) {
                                        Ok(()) => {
                                            new_topic_name.set(String::new());
                                            topics_message
                                                .set(Some((tid!("category-topic-added"), Color::Success)));
                                        }
                                        Err(e) => {
                                            error!("set_category_topics (add) failed: {e:?}");
                                            topics_message.set(Some((format!("{}: {e:?}", tid!("category-topic-error")), Color::Danger)));
                                        }
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
