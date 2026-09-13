use std::collections::HashSet;

use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::dioxus::{
    use_reducer_rename_category, use_reducer_set_topic_categories,
    use_table_visible_categories, use_table_visible_message_topic_categories,
};
use crate::module_bindings::{Category, MessageTopicCategory};

/// Live category names currently assigned to `topic_id`.
pub fn current_assigned_category_names(
    topic_id: u64,
    categories: &[Category],
    links: &[MessageTopicCategory],
) -> Vec<String> {
    let assigned: HashSet<u64> = links
        .iter()
        .filter(|link| link.topic_id == topic_id)
        .map(|link| link.category_id)
        .collect();
    categories
        .iter()
        .filter(|c| assigned.contains(&c.id))
        .map(|c| c.name.clone())
        .collect()
}

/// One category row: reads the current name from the `visible_categories` signal so renames
/// show up immediately without relying on a frozen clone from the parent.
#[component]
pub fn CategoryCheckRow(
    topic_id: u64,
    category_id: u64,
    is_checked: bool,
    mut renaming_category_id: Signal<Option<u64>>,
    mut rename_draft: Signal<String>,
    mut categories_message: Signal<Option<(String, Color)>>,
) -> Element {
    let categories = use_table_visible_categories();
    let topic_categories = use_table_visible_message_topic_categories();
    let set_topic_categories = use_reducer_set_topic_categories();
    let rename_category_key = use_reducer_rename_category();
    let rename_category_blur = rename_category_key.clone();

    let category_name = use_memo(move || {
        categories()
            .into_iter()
            .find(|c| c.id == category_id)
            .map(|c| c.name)
            .unwrap_or_default()
    });

    let name = category_name();
    if name.is_empty() && categories().iter().all(|c| c.id != category_id) {
        return rsx! {};
    }

    let is_renaming = renaming_category_id() == Some(category_id);

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
                            renaming_category_id.set(None);
                            return;
                        }
                        if e.key() != Key::Enter {
                            return;
                        }
                        let new_name = rename_draft.read().trim().to_string();
                        if new_name.is_empty() {
                            categories_message
                                .set(Some((tid!("topic-category-name-empty"), Color::Danger)));
                            return;
                        }
                        if new_name == category_name() {
                            renaming_category_id.set(None);
                            return;
                        }
                        match rename_category_key(category_id, new_name) {
                            Ok(()) => {
                                renaming_category_id.set(None);
                                categories_message
                                    .set(Some((tid!("topic-category-renamed"), Color::Success)));
                            }
                            Err(e) => {
                                error!("rename_category failed: {e:?}");
                                categories_message.set(Some((tid!("topic-category-error", error: format!("{e:?}")), Color::Danger)));
                            }
                        }
                    },
                    onblur: move |_| {
                        if renaming_category_id() != Some(category_id) {
                            return;
                        }
                        let new_name = rename_draft.read().trim().to_string();
                        if new_name.is_empty() || new_name == category_name() {
                            renaming_category_id.set(None);
                            return;
                        }
                        match rename_category_blur(category_id, new_name) {
                            Ok(()) => {
                                renaming_category_id.set(None);
                                categories_message
                                    .set(Some((tid!("topic-category-renamed"), Color::Success)));
                            }
                            Err(e) => {
                                error!("rename_category failed: {e:?}");
                                categories_message.set(Some((tid!("topic-category-error", error: format!("{e:?}")), Color::Danger)));
                            }
                        }
                    },
                }
            } else {
                Checkbox {
                    input_id: "category-check-{category_id}",
                    class: "mb-0 flex-grow-1",
                    checked: is_checked,
                    label: name.clone(),
                    onchange: move |_| {
                        let mut next = current_assigned_category_names(
                            topic_id,
                            &categories(),
                            &topic_categories(),
                        );
                        let current_name = category_name();
                        if is_checked {
                            next.retain(|n| n != &current_name);
                        } else if !current_name.is_empty() {
                            next.push(current_name);
                        }
                        next.sort();
                        next.dedup();
                        info!("Setting categories for topic {topic_id}: {next:?}");
                        match set_topic_categories(topic_id, next) {
                            Ok(()) => categories_message.set(None),
                            Err(e) => {
                                error!("set_topic_categories failed: {e:?}");
                                categories_message.set(Some((tid!("topic-category-error", error: format!("{e:?}")), Color::Danger)));
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
                        rename_draft.set(category_name());
                        renaming_category_id.set(Some(category_id));
                    },
                    Icon { name: "pencil" }
                }
            }
        }
    }
}

/// Card managing category assignments and adding new category tags.
#[component]
pub fn TopicCategoriesCard(
    topic_id: u64,
    assigned_category_ids: HashSet<u64>,
    category_ids: Vec<u64>,
    mut categories_message: Signal<Option<(String, Color)>>,
    mut new_category_name: Signal<String>,
    renaming_category_id: Signal<Option<u64>>,
    rename_draft: Signal<String>,
) -> Element {
    let categories = use_table_visible_categories();
    let topic_categories = use_table_visible_message_topic_categories();
    let set_topic_categories = use_reducer_set_topic_categories();

    rsx! {
        Card {
            class: "shadow-sm h-100",
            header_class: "bg-primary text-white",
            header: rsx! {
                h5 { class: "card-title mb-0",
                    Icon { name: "bookmark-star", class: "me-2" }
                    {tid!("topic-categories-title")}
                }
            },
            body: rsx! {
                if let Some((msg, color)) = categories_message.read().clone() {
                    Alert { color, class: "mb-3", "{msg}" }
                }
                p { class: "text-muted small mb-3",
                    {tid!("topic-categories-description")}
                }
                if category_ids.is_empty() {
                    div { class: "text-muted mb-3",
                        Icon { name: "inbox", class: "me-2" }
                        {tid!("topic-categories-empty")}
                    }
                } else {
                    ListGroup { tag: "div", class: "mb-3",
                        for category_id in category_ids.iter().copied() {
                            CategoryCheckRow {
                                key: "{category_id}",
                                topic_id,
                                category_id,
                                is_checked: assigned_category_ids.contains(&category_id),
                                renaming_category_id,
                                rename_draft,
                                categories_message,
                            }
                        }
                    }
                }
                InputGroup {
                    {
                        let set_categories_enter = set_topic_categories.clone();
                        let set_categories_click = set_topic_categories.clone();
                        rsx! {
                            Input {
                                r#type: "text",
                                placeholder: tid!("topic-categories-placeholder"),
                                value: "{new_category_name}",
                                oninput: move |e: FormEvent| new_category_name.set(e.value()),
                                onkeydown: move |e: KeyboardEvent| {
                                    if e.key() == Key::Enter {
                                        let name = new_category_name.read().trim().to_string();
                                        if name.is_empty() {
                                            return;
                                        }
                                        let mut next = current_assigned_category_names(
                                             topic_id,
                                             &categories(),
                                             &topic_categories(),
                                        );
                                        next.push(name);
                                        next.sort();
                                        next.dedup();
                                        match set_categories_enter(topic_id, next) {
                                            Ok(()) => {
                                                new_category_name.set(String::new());
                                                categories_message
                                                    .set(
                                                        Some((
                                                            tid!("topic-category-added"),
                                                            Color::Success,
                                                        )),
                                                    );
                                            }
                                            Err(e) => {
                                                error!("set_topic_categories (add) failed: {e:?}");
                                                categories_message.set(Some((tid!("topic-category-error", error: format!("{e:?}")), Color::Danger)));
                                            }
                                        }
                                    }
                                },
                            }
                            Button {
                                color: Color::Success,
                                disabled: new_category_name.read().trim().is_empty(),
                                onclick: move |_| {
                                    let name = new_category_name.read().trim().to_string();
                                    if name.is_empty() {
                                        return;
                                    }
                                    let mut next = current_assigned_category_names(
                                        topic_id,
                                        &categories(),
                                        &topic_categories(),
                                    );
                                    next.push(name);
                                    next.sort();
                                    next.dedup();
                                    match set_categories_click(topic_id, next) {
                                        Ok(()) => {
                                            new_category_name.set(String::new());
                                            categories_message
                                                .set(Some((tid!("topic-category-added"), Color::Success)));
                                        }
                                        Err(e) => {
                                            error!("set_topic_categories (add) failed: {e:?}");
                                            categories_message.set(Some((tid!("topic-category-error", error: format!("{e:?}")), Color::Danger)));
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
