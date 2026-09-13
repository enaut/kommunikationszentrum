use std::collections::HashSet;

use ::dioxus::prelude::*;
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::dioxus::{
    use_subscription, use_table_visible_mail_messages,
    use_table_visible_message_topics, use_table_visible_messages,
    use_table_visible_subscriptions, use_table_visible_account_configs,
    use_reducer_update_account_config,
    use_table_total_messages, use_table_topic_message_counts,
};
use crate::module_bindings::{MailMessage, ReceivedMessage};
use crate::oauth::UserInfo;
use spacetimedb_sdk::Timestamp;

/// Combined message data that joins ReceivedMessage with its MailMessage content
#[derive(Clone, Debug)]
struct MessageWithContent {
    received_message: ReceivedMessage,
    mail_message: MailMessage,
}

impl MessageWithContent {
    fn subject(&self) -> String {
        crate::mime_parser::decode_header_value("Subject", &self.mail_message.subject)
    }

    fn from_header(&self) -> String {
        crate::mime_parser::decode_header_value("From", &self.mail_message.from_header)
    }

    fn topic_email(&self) -> String {
        self.received_message.topic_email.clone()
    }

    fn topic_id(&self) -> u64 {
        self.received_message.topic_id
    }

    fn received_at(&self) -> Timestamp {
        self.received_message.received_at
    }

    fn cc_header(&self) -> Option<String> {
        self.mail_message
            .cc_header
            .as_deref()
            .map(|cc| crate::mime_parser::decode_header_value("Cc", cc))
    }

    fn date_header(&self) -> Option<String> {
        self.mail_message.date_header.clone()
    }

    fn message_id(&self) -> Option<String> {
        self.mail_message.message_id.clone()
    }

    fn reply_to(&self) -> Option<String> {
        self.mail_message.reply_to.clone()
    }

    fn body_raw(&self) -> String {
        self.mail_message.body_raw.clone()
    }

    fn body_decoded(&self) -> String {
        crate::mime_parser::decode_body(&self.mail_message.body_raw, &self.mail_message.headers_raw)
    }

    fn body_html_rendered(&self) -> String {
        crate::mime_parser::render_markdown_to_html(&self.body_decoded())
    }
}

// ---------------------------------------------------------------------------
// Visual helpers
// ---------------------------------------------------------------------------

fn topic_badge_color(topic_id: u64) -> Color {
    match topic_id % 5 {
        0 => Color::Primary,
        1 => Color::Success,
        2 => Color::Info,
        3 => Color::Warning,
        _ => Color::Secondary,
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[component]
pub fn MessagesPage(user_info: UserInfo) -> Element {

    // We subscribe to messages and mail_messages instead of calling reducers
    use_subscription(&[
        "SELECT * FROM visible_messages",
        "SELECT * FROM visible_mail_messages",
        "SELECT * FROM visible_message_topics",
        "SELECT * FROM visible_subscriptions",
        "SELECT * FROM visible_account_configs",
        "SELECT * FROM total_messages",
        "SELECT * FROM topic_message_counts",
    ]);

    let messages = use_table_visible_messages();
    let received_messages = messages;
    let mail_messages = use_table_visible_mail_messages();
    let topics = use_table_visible_message_topics();
    let subscriptions = use_table_visible_subscriptions();
    let configs = use_table_visible_account_configs();
    let total_messages_table = use_table_total_messages();
    let topic_message_counts_table = use_table_topic_message_counts();
    let update_config = use_reducer_update_account_config();

    let account_id: u64 = user_info.mitgliedsnr.parse().unwrap_or(0);
    let config = configs().into_iter().next();
    let filter_topic = config.as_ref().and_then(|c| c.selected_message_topic);
    let current_offset = config.as_ref().map(|c| c.message_offset).unwrap_or(0);
    let current_limit = config.as_ref().map(|c| c.message_limit).unwrap_or(50);
    
    let total_msgs = if let Some(top_id) = filter_topic {
        topic_message_counts_table()
            .into_iter()
            .find(|c| c.topic_id == top_id)
            .map(|c| c.count as u32)
            .unwrap_or(0)
    } else {
        total_messages_table()
            .into_iter()
            .next()
            .map(|r| r.count as u32)
            .unwrap_or_else(|| received_messages().len() as u32)
    };

    // Join ReceivedMessage with MailMessage using mail_message_id
    let messages_with_content: Vec<MessageWithContent> = received_messages()
        .into_iter()
        .filter_map(|received_msg| {
            mail_messages()
                .iter()
                .find(|mail_msg| mail_msg.id == received_msg.mail_message_id)
                .map(|mail_msg| MessageWithContent {
                    received_message: received_msg,
                    mail_message: mail_msg.clone(),
                })
        })
        .collect();

    let mut selected_id: Signal<Option<u64>> = use_signal(|| None);
    let mut show_raw_body: Signal<bool> = use_signal(|| false);

    // Only offer filter chips for topics the current account is actively
    // subscribed to
    let subscribed_topic_ids: HashSet<u64> = subscriptions()
        .into_iter()
        .filter(|s| {
            s.subscriber_account_id == account_id && crate::pages::is_active_subscription(&s.status)
        })
        .map(|s| s.topic_id)
        .collect();

    // Since the server already sorts, filters, and slices, we can just use the returned messages.
    // However, the `visible_messages` view sorts by DESC.
    let mut filtered = messages_with_content.clone();
    filtered.sort_by(|a, b| {
        let a_us = a.received_at();
        let b_us = b.received_at();
        b_us.cmp(&a_us)
    });

    let selected_msg = selected_id().and_then(|id| {
        filtered
            .iter()
            .find(|m| m.received_message.id == id)
            .cloned()
    });

    rsx! {
        Container { fluid: true, class: "mt-4",

            // ── Header ────────────────────────────────────────────────────
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "envelope-fill", class: "me-2" }
                        {tid!("messages-page-title")}
                    }
                    p { class: "text-muted mt-1",
                        Badge { color: Color::Primary, class: "me-2", "{messages_with_content.len()}" }
                        {tid!("messages-summary")}
                    }
                }
            }

            // ── Topic filter chips ──────────────────────────────────────
            Row { class: "mb-3",
                Col {
                    div { class: "d-flex flex-wrap gap-2 align-items-center",
                        span { class: "text-muted small me-1", {tid!("messages-filter")} }
                        Button {
                            color: if filter_topic.is_none() { Color::Primary } else { Color::Secondary },
                            outline: filter_topic.is_some(),
                            size: Size::Sm,
                            onclick: {
                                let update_config = update_config.clone();
                                move |_| {
                                    let _ = update_config(None, None, None, true, None, None, None, false, None, false, None, None);
                                    selected_id.set(None);
                                }
                            },
                            {tid!("messages-filter-all")}
                        }
                        for top in topics()
                            .into_iter()
                            .filter(|t| t.active && subscribed_topic_ids.contains(&t.id))
                        {
                            {
                                let top_id = top.id;
                                let is_active = filter_topic == Some(top_id);
                                rsx! {
                                    Button {
                                        color: topic_badge_color(top_id),
                                        outline: !is_active,
                                        size: Size::Sm,
                                        onclick: {
                                            let update_config = update_config.clone();
                                            move |_| {
                                                let _ = update_config(Some(0), None, Some(top_id), false, None, None, None, false, None, false, None, None);
                                                selected_id.set(None);
                                            }
                                        },
                                        "{top.name}"
                                    }
                                }
                            }
                        }
                        div { class: "ms-auto d-flex align-items-center gap-2",
                            Button {
                                color: Color::Secondary,
                                outline: true,
                                size: Size::Sm,
                                disabled: current_offset == 0,
                                onclick: {
                                    let update_config = update_config.clone();
                                    move |_| {
                                        if current_offset >= current_limit {
                                            let _ = update_config(Some(current_offset - current_limit), None, None, false, None, None, None, false, None, false, None, None);
                                        } else {
                                            let _ = update_config(Some(0), None, None, false, None, None, None, false, None, false, None, None);
                                        }
                                    }
                                },
                                Icon { name: "chevron-left" }
                            }
                            span { class: "text-muted small",
                                {tid!("pagination-page", page: (current_offset / current_limit) + 1)}
                            }
                            Button {
                                color: Color::Secondary,
                                outline: true,
                                size: Size::Sm,
                                disabled: current_offset + current_limit >= total_msgs,
                                onclick: {
                                    let update_config = update_config.clone();
                                    move |_| {
                                        let _ = update_config(Some(current_offset + current_limit), None, None, false, None, None, None, false, None, false, None, None);
                                    }
                                },
                                Icon { name: "chevron-right" }
                            }
                        }
                    }
                }
            }

            // ── Empty state ───────────────────────────────────────────────
            if filtered.is_empty() {
                Alert { color: Color::Info,
                    Icon { name: "inbox", class: "me-2" }
                    if filter_topic.is_none() {
                        {tid!("messages-empty")}
                    } else {
                        {tid!("messages-empty-topic")}
                    }
                }
            } else {

                // ── Two-column layout ──────────────────────────────────────
                Row {
                    // ── Message list ───────────────────────────────────────
                    Col { md: ColumnSize::Span(4), class: "mb-3",
                        Card {
                            class: "shadow-sm",
                            body_class: "p-0",
                            body: rsx! {
                                ListGroup { flush: true,
                                    for msg in filtered.clone() {
                                        {
                                            let msg_id = msg.received_message.id;
                                            let is_sel = selected_id() == Some(msg_id);
                                            let subject = if msg.subject().is_empty() {
                                                tid!("messages-no-subject")
                                            } else {
                                                msg.subject()
                                            };
                                            let sender = msg.from_header();
                                            let top_email = msg.topic_email();
                                            let date_str =
                                                msg.received_at().to_string();
                                            let badge_color = topic_badge_color(msg.topic_id());
                                            rsx! {
                                                ListGroupItem {
                                                    active: is_sel,
                                                    class: "px-3 py-2",
                                                    onclick: move |_| selected_id.set(Some(msg_id)),
                                                    div { class: "d-flex justify-content-between align-items-start mb-1",
                                                        Badge {
                                                            color: badge_color,
                                                            class: "text-truncate",
                                                            style: "max-width: 10rem;",
                                                            "{top_email}"
                                                        }
                                                        small { class: if is_sel { "text-white-50 text-nowrap ms-2" } else { "text-muted text-nowrap ms-2" },
                                                            "{date_str}"
                                                        }
                                                    }
                                                    div { class: "fw-semibold small text-truncate", "{subject}" }
                                                    div { class: if is_sel { "small text-white-50 text-truncate" } else { "small text-muted text-truncate" },
                                                        "{sender}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                        }
                    }

                    // ── Detail panel ───────────────────────────────────────
                    Col { md: ColumnSize::Span(8), class: "mb-3",
                        if let Some(msg) = selected_msg {
                            Card {
                                class: "shadow-sm",
                                header: rsx! {
                                    div { class: "d-flex align-items-center gap-2 flex-wrap",
                                        Badge { color: topic_badge_color(msg.topic_id()), "{msg.topic_email()}" }
                                        span { class: "fw-semibold",
                                            if msg.subject().is_empty() {
                                                {tid!("messages-no-subject")}
                                            } else {
                                                "{msg.subject()}"
                                            }
                                        }
                                        small { class: "text-muted ms-auto", {msg.received_at().to_string()} }
                                    }
                                },
                                body: rsx! {
                                    // ── Parsed header fields ───────────────
                                    Table {
                                    size: Size::Sm,
                                    borderless: true,
                                    class: "mb-0",
                                        tbody {
                                            tr {
                                                th {
                                                    class: "text-muted small pe-3",
                                                    style: "width: 5.5rem; white-space: nowrap;",
                                                    {tid!("messages-header-from")}
                                                }
                                                td { class: "small", "{msg.from_header()}" }
                                            }
                                            tr {
                                                th { class: "text-muted small pe-3", {tid!("messages-header-to")} }
                                                td { class: "small", "{msg.topic_email()}" }
                                            }
                                            if let Some(cc) = msg.cc_header() {
                                                tr {
                                                    th { class: "text-muted small pe-3", {tid!("messages-header-cc")} }
                                                    td { class: "small", "{cc}" }
                                                }
                                            }
                                            if let Some(date) = msg.date_header() {
                                                tr {
                                                    th { class: "text-muted small pe-3", {tid!("messages-header-date")} }
                                                    td { class: "small", "{date}" }
                                                }
                                            }
                                            if let Some(mid) = msg.message_id() {
                                                tr {
                                                    th { class: "text-muted small pe-3", {tid!("messages-header-message-id")} }
                                                    td { class: "small font-monospace text-break", "{mid}" }
                                                }
                                            }
                                            if let Some(rt) = msg.reply_to() {
                                                tr {
                                                    th { class: "text-muted small pe-3", {tid!("messages-header-reply-to")} }
                                                    td { class: "small", "{rt}" }
                                                }
                                            }
                                        }
                                    }
                                    hr { class: "my-3" }
                                    // ── Body ──────────────────────────────
                                    if msg.body_raw().is_empty() {
                                        Alert { color: Color::Warning, class: "small mb-0",
                                            Icon { name: "exclamation-triangle", class: "me-1" }
                                            {tid!("messages-body-empty")}
                                        }
                                    } else {
                                        div { class: "d-flex justify-content-end mb-2",
                                            div { class: "btn-group btn-group-sm",
                                                button {
                                                    r#type: "button",
                                                    class: if !show_raw_body() { "btn btn-outline-primary active" } else { "btn btn-outline-secondary" },
                                                    onclick: move |_| show_raw_body.set(false),
                                                    {tid!("messages-view-rendered")}
                                                }
                                                button {
                                                    r#type: "button",
                                                    class: if show_raw_body() { "btn btn-outline-primary active" } else { "btn btn-outline-secondary" },
                                                    onclick: move |_| show_raw_body.set(true),
                                                    {tid!("messages-view-raw")}
                                                }
                                            }
                                        }
                                        if show_raw_body() {
                                            pre {
                                                class: "small bg-body-secondary rounded p-3 mb-0 overflow-auto font-monospace",
                                                style: "max-height: 28rem; white-space: pre-wrap; word-break: break-word;",
                                                "{msg.body_raw()}"
                                            }
                                        } else {
                                            div {
                                                class: "small bg-body-secondary rounded p-3 mb-0 overflow-auto markdown-body",
                                                style: "max-height: 28rem; word-break: break-word;",
                                                dangerous_inner_html: "{msg.body_html_rendered()}"
                                            }
                                        }
                                    }
                                },
                            }
                        } else {
                            // Placeholder when nothing is selected
                            Card {
                                class: "shadow-sm",
                                body: rsx! {
                                    div { class: "d-flex flex-column align-items-center justify-content-center py-5 text-muted",
                                        Icon { name: "envelope-open", class: "display-6 mb-3" }
                                        p { class: "mb-0", {tid!("messages-select-placeholder")} }
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
