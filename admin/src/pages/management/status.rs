use ::dioxus::{logger::tracing::error, prelude::*};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

use crate::module_bindings::dioxus::{
    ConnectionState, use_connection_error, use_connection_state, use_subscription,
    use_table_sender_mail_delivery_claimed, use_table_sender_mail_delivery_done,
    use_table_sender_mail_delivery_events, use_table_sender_mail_delivery_messages,
    use_table_sender_mail_delivery_pending, use_table_sender_mail_delivery_temporary_failed,
    use_table_sender_mail_ingress, use_table_sender_mail_messages,
    use_table_sender_system_mail_pending,
};
use crate::module_bindings::{
    DeliveryFinalState, DeliveryStatus, MailDeliveryMessage, SystemMailPending,
};
use crate::oauth::UserInfo;

#[component]
pub fn ManagementStatusPage(user_info: UserInfo) -> Element {
    rsx! {
        Container { fluid: true, class: "mt-4",
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "activity", class: "me-2" }
                        {tid!("management-status-title")}
                    }
                }
            }
            ConnectionStatusCard { user_info: user_info.clone() }

            Row { class: "mt-4 mb-2",
                Col {
                    h4 { class: "text-secondary d-flex align-items-center mb-0",
                        Icon { name: "envelope-paper", class: "me-2" }
                        {tid!("status-section-system-mails")}
                    }
                }
            }
            SystemMailCard {}

            Row { class: "mt-5 mb-2",
                Col {
                    h4 { class: "text-secondary d-flex align-items-center mb-0",
                        Icon { name: "send", class: "me-2" }
                        {tid!("status-section-fanout-mails")}
                    }
                }
            }
            MailIngressCard {}
            TemporaryFailedCard {}
            DeliveryEventsCard {}
            PendingCard {}
            ClaimedCard {}
            DoneCard {}
        }
    }
}

#[component]
fn ConnectionStatusCard(user_info: UserInfo) -> Element {
    use_subscription(&["SELECT * FROM visible_admin_identities"]);
    let state = use_connection_state();
    let conn_error = use_connection_error();
    let (alert_color, icon_name, status_text): (Color, &'static str, String) = match state() {
        ConnectionState::Connected(id, _) => (Color::Primary, "check-circle-fill", format!("{id}")),
        ConnectionState::Connecting => (
            Color::Info,
            "arrow-repeat",
            tid!("status-connection-connecting"),
        ),
        ConnectionState::Reconnecting { attempt, delay_ms } => (
            Color::Warning,
            "exclamation-triangle-fill",
            tid!("status-connection-reconnecting", attempt: attempt, delay_ms: delay_ms),
        ),
        ConnectionState::Error => (
            Color::Danger,
            "exclamation-circle-fill",
            tid!("status-connection-error"),
        ),
        ConnectionState::Disconnected => (
            Color::Secondary,
            "circle-fill",
            tid!("status-connection-disconnected"),
        ),
    };

    rsx! {
        Row { class: "mb-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-{alert_color} text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "plug-fill", class: "me-2" }
                            {tid!("status-connection-title")}
                            Icon { name: icon_name, class: "mx-3" }
                        }
                    },
                    body: rsx! {
                        Row { class: "text-center",
                            Col { md: ColumnSize::Span(4),
                                div { class: "border-end",
                                    h6 { class: "text-muted mb-1", {tid!("status-identity")} }
                                    p { class: "h5 mb-0", "{status_text}" }
                                    if let Some(err) = conn_error() {
                                        div { class: "text-danger mt-1 small",
                                            {tid!("status-connection-error-details", error: err)}
                                        }
                                    }
                                }
                            }
                            Col { md: ColumnSize::Span(4),
                                div { class: "border-end",
                                    h6 { class: "text-muted mb-1", {tid!("status-member-number")} }
                                    p { class: "h5 mb-0", "{user_info.subject_id}" }
                                }
                            }
                            Col { md: ColumnSize::Span(4),
                                div { class: "border-end",
                                    h6 { class: "text-muted mb-1", {tid!("status-email")} }
                                    p { class: "h5 mb-0",
                                        if let Some(email) = &user_info.email {
                                            "{email}"
                                        } else {
                                            {tid!("status-email-empty")}
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

#[component]
fn MailIngressCard() -> Element {
    use_subscription(&[
        "SELECT * FROM sender_mail_ingress",
        "SELECT * FROM sender_mail_messages",
    ]);
    let ingress_list = use_table_sender_mail_ingress();
    let mail_messages = use_table_sender_mail_messages();

    let mut show_detail_modal = use_signal(|| false);
    let mut selected_ingress_id = use_signal(|| None::<String>);

    let mut rows = ingress_list();
    rows.sort_by(|a, b| b.claim.updated_at.cmp(&a.claim.updated_at));
    let msg_list = mail_messages();

    rsx! {
        Row { class: "mt-2",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-primary text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "inbox-fill", class: "me-2" }
                            {tid!("status-ingress-title")}
                            span { class: "badge bg-white text-primary ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0",
                                Icon { name: "inbox", class: "me-2" }
                                {tid!("status-ingress-empty")}
                            }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0 align-middle",
                                thead { class: "table-light",
                                    tr {
                                        th { {tid!("status-ingress-id")} }
                                        th { {tid!("status-ingress-status")} }
                                        th { {tid!("status-ingress-topic")} }
                                        th { {tid!("status-ingress-subject")} }
                                        th { {tid!("status-ingress-sender")} }
                                        th { {tid!("status-ingress-progress")} }
                                        th { {tid!("status-ingress-worker")} }
                                        th { {tid!("status-ingress-time")} }
                                        th { class: "text-end", {tid!("status-ingress-actions")} }
                                    }
                                }
                                tbody {
                                    for ingress in &rows {
                                        {
                                            let ingress_id = ingress.id.clone();
                                            let msg = msg_list.iter().find(|m| m.id == ingress.mail_message_id);
                                            let subject = msg.map(|m| m.subject.as_str()).unwrap_or("–");
                                            let sender = msg.map(|m| m.sender_email.as_str()).unwrap_or("–");
                                            let worker = ingress.claim.instance_id.as_deref().unwrap_or("–");
                                            let time_str = if ingress.completed_at != spacetimedb_sdk::Timestamp::UNIX_EPOCH {
                                                ingress.completed_at.to_string()
                                            } else {
                                                ingress.claim.updated_at.to_string()
                                            };
                                            let (status_class, status_label) = match ingress.claim.status {
                                                DeliveryStatus::Pending => ("badge bg-secondary", tid!("status-ingress-status-pending")),
                                                DeliveryStatus::Processing => ("badge bg-info text-white", tid!("status-ingress-status-processing")),
                                                DeliveryStatus::Completed => ("badge bg-success", tid!("status-ingress-status-completed")),
                                                DeliveryStatus::RetryScheduled => ("badge bg-warning text-dark", tid!("status-ingress-status-retry-scheduled")),
                                                DeliveryStatus::Failed => ("badge bg-danger", tid!("status-ingress-status-failed")),
                                                other => ("badge bg-dark text-white", format!("{other:?}")),
                                            };
                                            rsx! {
                                                tr {
                                                    td {
                                                        code { class: "small text-break", "{ingress.id}" }
                                                    }
                                                    td {
                                                        span { class: "{status_class}", "{status_label}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{ingress.topic_email}" }
                                                    }
                                                    td {
                                                        div { class: "small text-break", "{subject}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{sender}" }
                                                    }
                                                    td {
                                                        span { class: "badge bg-light text-dark border",
                                                            "{ingress.delivery_count} / {ingress.recipient_count}"
                                                        }
                                                        if ingress.failed_delivery_count > 0 {
                                                            span { class: "badge bg-danger ms-1",
                                                                {tid!("status-ingress-err", count: ingress.failed_delivery_count)}
                                                            }
                                                        }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{worker}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{time_str}" }
                                                    }
                                                    td { class: "text-end",
                                                        {
                                                            let id_for_click = ingress_id.clone();
                                                            rsx! {
                                                                Button {
                                                                    color: Color::Secondary,
                                                                    outline: true,
                                                                    size: Size::Sm,
                                                                    onclick: move |_| {
                                                                        selected_ingress_id.set(Some(id_for_click.clone()));
                                                                        show_detail_modal.set(true);
                                                                    },
                                                                    Icon { name: "eye", class: "me-1" }
                                                                    {tid!("status-ingress-view")}
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
                    },
                }
            }
        }
        {
            let target_opt = selected_ingress_id().and_then(|id| rows.iter().find(|r| r.id == id).cloned());
            if show_detail_modal() && selected_ingress_id().is_some() && target_opt.is_none() {
                show_detail_modal.set(false);
                selected_ingress_id.set(None);
            }
            if let Some(target) = target_opt {
                let msg = msg_list.iter().find(|m| m.id == target.mail_message_id);
                let subject = msg.map(|m| m.subject.as_str()).unwrap_or("–");
                let sender = msg.map(|m| m.sender_email.as_str()).unwrap_or("–");
                let body_raw = msg.map(|m| m.body_raw.as_str()).unwrap_or("–");
                let (status_class, status_label) = match target.claim.status {
                    DeliveryStatus::Pending => ("badge bg-secondary", tid!("status-ingress-status-pending")),
                    DeliveryStatus::Processing => ("badge bg-info text-white", tid!("status-ingress-status-processing")),
                    DeliveryStatus::Completed => ("badge bg-success", tid!("status-ingress-status-completed")),
                    DeliveryStatus::RetryScheduled => ("badge bg-warning text-dark", tid!("status-ingress-status-retry-scheduled")),
                    DeliveryStatus::Failed => ("badge bg-danger", tid!("status-ingress-status-failed")),
                    other => ("badge bg-dark text-white", format!("{other:?}")),
                };
                let completed_str = if target.completed_at != spacetimedb_sdk::Timestamp::UNIX_EPOCH {
                    target.completed_at.to_string()
                } else {
                    "–".to_string()
                };
                rsx! {
                    Modal {
                        show: show_detail_modal,
                        title: tid!("status-ingress-detail-title"),
                        body: rsx! {
                            div { class: "mb-3",
                                strong { "{tid!(\"status-ingress-id\")}: " }
                                code { "{target.id}" }
                            }
                            div { class: "mb-3",
                                strong { "{tid!(\"status-ingress-status\")}: " }
                                span { class: "{status_class} ms-2", "{status_label}" }
                                span { class: "ms-2 small text-muted",
                                    if target.failed_delivery_count > 0 {
                                        {tid!("status-ingress-enqueued-failed",
                                            delivered: target.delivery_count,
                                            total: target.recipient_count,
                                            failed: target.failed_delivery_count
                                        )}
                                    } else {
                                        {tid!("status-ingress-enqueued",
                                            delivered: target.delivery_count,
                                            total: target.recipient_count
                                        )}
                                    }
                                }
                            }
                            div { class: "mb-3",
                                strong { "{tid!(\"status-ingress-topic\")}: " }
                                span { class: "text-muted ms-2", "{target.topic_email}" }
                            }
                            div { class: "mb-3",
                                strong { "{tid!(\"status-ingress-sender\")}: " }
                                span { class: "text-muted ms-2", "{sender}" }
                            }
                            div { class: "mb-3",
                                strong { "{tid!(\"status-ingress-subject\")}: " }
                                span { class: "text-muted ms-2", "{subject}" }
                            }
                            if let Some(worker) = target.claim.instance_id.as_deref() {
                                div { class: "mb-3",
                                    strong { "{tid!(\"status-ingress-worker\")}: " }
                                    span { class: "badge bg-info text-white ms-2", "{worker}" }
                                }
                            }
                            div { class: "mb-3",
                                strong { "{tid!(\"status-last-updated\")}: " }
                                span { class: "text-muted ms-2", "{target.claim.updated_at}" }
                            }
                            if completed_str != "–" {
                                div { class: "mb-3",
                                    strong { "{tid!(\"status-ingress-status-completed\")}: " }
                                    span { class: "text-muted ms-2", "{completed_str}" }
                                }
                            }
                            if let Some(err) = target.claim.last_error.as_deref() {
                                div { class: "alert alert-danger py-2 px-3 small text-break mt-2",
                                    "{err}"
                                }
                            }
                            div { class: "mb-0",
                                strong { "{tid!(\"status-ingress-body\")}:" }
                                pre {
                                    class: "bg-light p-3 rounded mt-2 text-break border",
                                    style: "white-space: pre-wrap; font-size: 0.875rem; max-height: 400px; overflow-y: auto;",
                                    "{body_raw}"
                                }
                            }
                        },
                        footer: rsx! {
                            Button {
                                color: Color::Secondary,
                                onclick: move |_| {
                                    show_detail_modal.set(false);
                                    selected_ingress_id.set(None);
                                },
                                {tid!("status-ingress-close")}
                            }
                        }
                    }
                }
            } else {
                rsx! {}
            }
        }
    }
}

#[component]
fn DeliveryDetailModal(
    show: Signal<bool>,
    message: Signal<Option<MailDeliveryMessage>>,
) -> Element {
    if let Some(target) = message() {
        rsx! {
            Modal {
                show,
                title: tid!("status-delivery-detail-title"),
                body: rsx! {
                    div { class: "mb-3",
                        strong { "{tid!(\"status-temporary-failed-delivery-id\")}: " }
                        code { "{target.delivery_id}" }
                    }
                    div { class: "mb-3",
                        strong { "{tid!(\"status-pending-ingress\")}: " }
                        code { "{target.ingress_id}" }
                    }
                    div { class: "mb-3",
                        strong { "{tid!(\"status-temporary-failed-recipient\")}: " }
                        span { class: "text-muted ms-2", "{target.recipient_email}" }
                    }
                    div { class: "mb-3",
                        strong { "{tid!(\"status-delivery-from\")}: " }
                        span { class: "text-muted ms-2", "{target.from_header}" }
                    }
                    if !target.reply_to.is_empty() {
                        div { class: "mb-3",
                            strong { "{tid!(\"status-delivery-reply-to\")}: " }
                            span { class: "text-muted ms-2", "{target.reply_to}" }
                        }
                    }
                    div { class: "mb-0",
                        strong { "{tid!(\"status-delivery-raw-message\")}:" }
                        pre {
                            class: "bg-light p-3 rounded mt-2 text-break border",
                            style: "white-space: pre-wrap; font-size: 0.875rem; max-height: 400px; overflow-y: auto;",
                            "{target.raw_message}"
                        }
                    }
                },
                footer: rsx! {
                    Button {
                        color: Color::Secondary,
                        onclick: move |_| show.set(false),
                        {tid!("status-delivery-close")}
                    }
                }
            }
        }
    } else {
        rsx! {}
    }
}

#[component]
fn TemporaryFailedCard() -> Element {
    use_subscription(&[
        "SELECT * FROM sender_mail_delivery_temporary_failed",
        "SELECT * FROM sender_mail_delivery_messages",
    ]);
    let temporary_failed = use_table_sender_mail_delivery_temporary_failed();
    let messages = use_table_sender_mail_delivery_messages();
    let cancel_retry = crate::module_bindings::dioxus::use_reducer_cancel_mail_delivery_retry();

    let mut show_detail_modal = use_signal(|| false);
    let mut selected_msg = use_signal(|| None::<MailDeliveryMessage>);

    let mut rows = temporary_failed();
    rows.sort_by(|a, b| a.delivery_id.cmp(&b.delivery_id));
    let msg_list = messages();

    rsx! {
        Row { class: "mt-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-warning text-dark",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "clock-history", class: "me-2" }
                            {tid!("status-temporary-failed-title")}
                            span { class: "badge bg-dark text-white ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0",
                                Icon { name: "inbox", class: "me-2" }
                                {tid!("status-temporary-failed-empty")}
                            }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0 align-middle",
                                thead { class: "table-light",
                                    tr {
                                        th { {tid!("status-temporary-failed-delivery-id")} }
                                        th { {tid!("status-temporary-failed-recipient")} }
                                        th { {tid!("status-temporary-failed-retry-at")} }
                                        th { {tid!("status-last-updated")} }
                                        th { {tid!("status-temporary-failed-reason")} }
                                        th { class: "text-end", {tid!("status-temporary-failed-action")} }
                                    }
                                }
                                tbody {
                                    for failed in rows {
                                        {
                                            let delivery_id = failed.delivery_id.clone();
                                            let cancel = cancel_retry.clone();
                                            let msg_item = msg_list
                                                .iter()
                                                .find(|m| m.delivery_id == failed.delivery_id)
                                                .cloned();
                                            let recipient = msg_item
                                                .as_ref()
                                                .map(|m| m.recipient_email.as_str())
                                                .unwrap_or("–");
                                            rsx! {
                                                tr {
                                                    td {
                                                        code { class: "small text-break", "{failed.delivery_id}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{recipient}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{failed.next_attempt_at}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{failed.last_updated}" }
                                                    }
                                                    td {
                                                        div { class: "small text-break", "{failed.last_error}" }
                                                    }
                                                    td { class: "text-end",
                                                        div { class: "d-inline-flex gap-1 justify-content-end",
                                                            if let Some(msg) = msg_item {
                                                                Button {
                                                                    color: Color::Secondary,
                                                                    outline: true,
                                                                    size: Size::Sm,
                                                                    onclick: move |_| {
                                                                        selected_msg.set(Some(msg.clone()));
                                                                        show_detail_modal.set(true);
                                                                    },
                                                                    Icon { name: "eye", class: "me-1" }
                                                                    {tid!("status-delivery-action-view")}
                                                                }
                                                            }
                                                            Button {
                                                                color: Color::Warning,
                                                                outline: true,
                                                                size: Size::Sm,
                                                                onclick: move |_| {
                                                                    if let Err(e) = cancel(delivery_id.clone()) {
                                                                        error!("cancel_mail_delivery_retry failed: {e:?}");
                                                                    }
                                                                },
                                                                Icon { name: "x-circle", class: "me-1" }
                                                                {tid!("status-temporary-failed-cancel")}
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
                    },
                }
            }
        }
        DeliveryDetailModal { show: show_detail_modal, message: selected_msg }
    }
}

#[component]
fn DeliveryEventsCard() -> Element {
    use_subscription(&["SELECT * FROM sender_mail_delivery_events"]);
    let delivery_events = use_table_sender_mail_delivery_events();

    let mut rows = delivery_events();
    rows.sort_by(|a, b| a.id.cmp(&b.id));

    rsx! {
        Row { class: "mt-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-dark text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "journal-text", class: "me-2" }
                            {tid!("status-delivery-events-title")}
                            span { class: "badge bg-white text-dark ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", {tid!("status-delivery-events-empty")} }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0",
                                thead { class: "table-light",
                                    tr {
                                        th { {tid!("status-delivery-events-id")} }
                                        th { {tid!("status-delivery-events-delivery-id")} }
                                        th { {tid!("status-delivery-events-type")} }
                                        th { {tid!("status-delivery-events-attempt")} }
                                        th { {tid!("status-delivery-events-details")} }
                                    }
                                }
                                tbody {
                                    for event in rows {
                                        tr {
                                            td {
                                                code { class: "small text-break", "{event.id}" }
                                            }
                                            td {
                                                code { class: "small text-break", "{event.delivery_id}" }
                                            }
                                            td {
                                                small { class: "text-muted", "{event.event_type}" }
                                            }
                                            td {
                                                small { class: "text-muted", "{event.attempt_no}" }
                                            }
                                            td {
                                                div { class: "small text-break", "{event.details}" }
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

#[component]
fn PendingCard() -> Element {
    use_subscription(&[
        "SELECT * FROM sender_mail_delivery_pending",
        "SELECT * FROM sender_mail_delivery_messages",
    ]);
    let pending_deliveries = use_table_sender_mail_delivery_pending();
    let messages = use_table_sender_mail_delivery_messages();

    let mut show_detail_modal = use_signal(|| false);
    let mut selected_msg = use_signal(|| None::<MailDeliveryMessage>);

    let mut rows = pending_deliveries();
    rows.sort_by(|a, b| a.delivery_id.cmp(&b.delivery_id));
    let msg_list = messages();

    rsx! {
        Row { class: "mt-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-secondary text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "hourglass-split", class: "me-2" }
                            {tid!("status-pending-title")}
                            span { class: "badge bg-white text-secondary ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", {tid!("status-pending-empty")} }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0 align-middle",
                                thead { class: "table-light",
                                    tr {
                                        th { {tid!("status-pending-id")} }
                                        th { {tid!("status-pending-recipient")} }
                                        th { {tid!("status-pending-ingress")} }
                                        th { {tid!("status-last-updated")} }
                                        th { class: "text-end", {tid!("status-temporary-failed-action")} }
                                    }
                                }
                                tbody {
                                    for row in rows {
                                        {
                                            let msg_item = msg_list
                                                .iter()
                                                .find(|m| m.delivery_id == row.delivery_id)
                                                .cloned();
                                            let recipient = msg_item
                                                .as_ref()
                                                .map(|m| m.recipient_email.as_str())
                                                .unwrap_or("–");
                                            rsx! {
                                                tr {
                                                    td {
                                                        code { class: "small text-break", "{row.delivery_id}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{recipient}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{row.ingress_id}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{row.last_updated}" }
                                                    }
                                                    td { class: "text-end",
                                                        if let Some(msg) = msg_item {
                                                            Button {
                                                                color: Color::Secondary,
                                                                outline: true,
                                                                size: Size::Sm,
                                                                onclick: move |_| {
                                                                    selected_msg.set(Some(msg.clone()));
                                                                    show_detail_modal.set(true);
                                                                },
                                                                Icon { name: "eye", class: "me-1" }
                                                                {tid!("status-delivery-action-view")}
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
                    },
                }
            }
        }
        DeliveryDetailModal { show: show_detail_modal, message: selected_msg }
    }
}

#[component]
fn ClaimedCard() -> Element {
    use_subscription(&[
        "SELECT * FROM sender_mail_delivery_claimed",
        "SELECT * FROM sender_mail_delivery_messages",
    ]);
    let claimed_deliveries = use_table_sender_mail_delivery_claimed();
    let messages = use_table_sender_mail_delivery_messages();

    let mut show_detail_modal = use_signal(|| false);
    let mut selected_msg = use_signal(|| None::<MailDeliveryMessage>);

    let mut rows = claimed_deliveries();
    rows.sort_by(|a, b| a.delivery_id.cmp(&b.delivery_id));
    let msg_list = messages();

    rsx! {
        Row { class: "mt-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-info text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "person-workspace", class: "me-2" }
                            {tid!("status-claimed-title")}
                            span { class: "badge bg-white text-info ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", {tid!("status-claimed-empty")} }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0 align-middle",
                                thead { class: "table-light",
                                    tr {
                                        th { {tid!("status-claimed-id")} }
                                        th { {tid!("status-claimed-worker")} }
                                        th { {tid!("status-claimed-lease")} }
                                        th { {tid!("status-claimed-recipient")} }
                                        th { {tid!("status-last-updated")} }
                                        th { class: "text-end", {tid!("status-temporary-failed-action")} }
                                    }
                                }
                                tbody {
                                    for row in rows {
                                        {
                                            let msg_item = msg_list
                                                .iter()
                                                .find(|m| m.delivery_id == row.delivery_id)
                                                .cloned();
                                            let recipient = msg_item
                                                .as_ref()
                                                .map(|m| m.recipient_email.as_str())
                                                .unwrap_or("–");
                                            rsx! {
                                                tr {
                                                    td {
                                                        code { class: "small text-break", "{row.delivery_id}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{row.worker}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{row.lease_expires_at}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{recipient}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{row.last_updated}" }
                                                    }
                                                    td { class: "text-end",
                                                        if let Some(msg) = msg_item {
                                                            Button {
                                                                color: Color::Secondary,
                                                                outline: true,
                                                                size: Size::Sm,
                                                                onclick: move |_| {
                                                                    selected_msg.set(Some(msg.clone()));
                                                                    show_detail_modal.set(true);
                                                                },
                                                                Icon { name: "eye", class: "me-1" }
                                                                {tid!("status-delivery-action-view")}
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
                    },
                }
            }
        }
        DeliveryDetailModal { show: show_detail_modal, message: selected_msg }
    }
}

#[component]
fn DoneCard() -> Element {
    use_subscription(&[
        "SELECT * FROM sender_mail_delivery_done",
        "SELECT * FROM sender_mail_delivery_messages",
    ]);
    let done_deliveries = use_table_sender_mail_delivery_done();
    let messages = use_table_sender_mail_delivery_messages();

    let mut show_detail_modal = use_signal(|| false);
    let mut selected_msg = use_signal(|| None::<MailDeliveryMessage>);

    let mut rows = done_deliveries();
    rows.sort_by(|a, b| a.delivery_id.cmp(&b.delivery_id));
    let msg_list = messages();

    rsx! {
        Row { class: "mt-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-success text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "check-circle", class: "me-2" }
                            {tid!("status-done-title")}
                            span { class: "badge bg-white text-success ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", {tid!("status-done-empty")} }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0 align-middle",
                                thead { class: "table-light",
                                    tr {
                                        th { {tid!("status-done-id")} }
                                        th { {tid!("status-done-status")} }
                                        th { {tid!("status-done-recipient")} }
                                        th { {tid!("status-done-reason")} }
                                        th { {tid!("status-last-updated")} }
                                        th { class: "text-end", {tid!("status-temporary-failed-action")} }
                                    }
                                }
                                tbody {
                                    for row in rows {
                                        {
                                            let (status_icon, status_color, status_text) = match row.final_state {
                                                DeliveryFinalState::Sent => ("check-circle-fill", "text-success", tid!("status-done-sent")),
                                                DeliveryFinalState::Failed => ("x-circle-fill", "text-danger", tid!("status-done-failed")),
                                                DeliveryFinalState::Bounced => ("envelope-slash-fill", "text-warning", tid!("status-done-bounced")),
                                                DeliveryFinalState::Cancelled => ("slash-circle", "text-secondary", tid!("status-done-cancelled")),
                                            };
                                            let msg_item = msg_list
                                                .iter()
                                                .find(|m| m.delivery_id == row.delivery_id)
                                                .cloned();
                                            let recipient = msg_item
                                                .as_ref()
                                                .map(|m| m.recipient_email.as_str())
                                                .unwrap_or("–");
                                            let reason = if let Some(ref err) = row.last_error {
                                                err.clone()
                                            } else {
                                                tid!("general-none")
                                            };
                                            rsx! {
                                                tr {
                                                    td {
                                                        code { class: "small text-break", "{row.delivery_id}" }
                                                    }
                                                    td {
                                                        span { class: "d-inline-flex align-items-center gap-2",
                                                            Icon { name: status_icon, class: format!("{status_color} fw-bold") }
                                                            small { class: "text-muted", "{status_text}" }
                                                        }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{recipient}" }
                                                    }
                                                    td {
                                                        small { class: "text-break text-muted", "{reason}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{row.last_updated}" }
                                                    }
                                                    td { class: "text-end",
                                                        if let Some(msg) = msg_item {
                                                            Button {
                                                                color: Color::Secondary,
                                                                outline: true,
                                                                size: Size::Sm,
                                                                onclick: move |_| {
                                                                    selected_msg.set(Some(msg.clone()));
                                                                    show_detail_modal.set(true);
                                                                },
                                                                Icon { name: "eye", class: "me-1" }
                                                                {tid!("status-delivery-action-view")}
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
                    },
                }
            }
        }
        DeliveryDetailModal { show: show_detail_modal, message: selected_msg }
    }
}

#[component]
fn SystemMailCard() -> Element {
    use_subscription(&["SELECT * FROM sender_system_mail_pending"]);
    let system_mails = use_table_sender_system_mail_pending();
    let release_mail = crate::module_bindings::dioxus::use_reducer_release_system_mail();
    let complete_mail = crate::module_bindings::dioxus::use_reducer_complete_system_mail();

    let mut show_detail_modal = use_signal(|| false);
    let mut selected_mail = use_signal(|| None::<SystemMailPending>);

    let mut rows = system_mails();
    rows.sort_by(|a, b| a.id.cmp(&b.id));

    rsx! {
        Row { class: "mt-2",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-primary text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "envelope-fill", class: "me-2" }
                            {tid!("status-system-mail-title")}
                            span { class: "badge bg-white text-primary ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0",
                                Icon { name: "inbox", class: "me-2" }
                                {tid!("status-system-mail-empty")}
                            }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0 align-middle",
                                thead { class: "table-light",
                                    tr {
                                        th { {tid!("status-system-mail-id")} }
                                        th { {tid!("status-system-mail-status")} }
                                        th { {tid!("status-system-mail-recipient")} }
                                        th { {tid!("status-system-mail-subject")} }
                                        th { {tid!("status-system-mail-worker")} }
                                        th { {tid!("status-system-mail-claimed-at")} }
                                        th { class: "text-end", {tid!("status-system-mail-actions")} }
                                    }
                                }
                                tbody {
                                    for mail in rows {
                                        {
                                            let mail_id = mail.id;
                                            let release = release_mail.clone();
                                            let complete = complete_mail.clone();
                                            let is_claimed = mail.instance_id.is_some();
                                            let worker = mail.instance_id.as_deref().unwrap_or("–");
                                            let claimed_at_str = mail
                                                .claimed_at
                                                .map(|ts| ts.to_string())
                                                .unwrap_or_else(|| "–".to_string());
                                            let mail_clone = mail.clone();
                                            rsx! {
                                                tr {
                                                    td {
                                                        code { class: "small text-break", "#{mail.id}" }
                                                    }
                                                    td {
                                                        if is_claimed {
                                                            span { class: "badge bg-info text-white",
                                                                {tid!("status-system-mail-status-claimed")}
                                                            }
                                                        } else {
                                                            span { class: "badge bg-secondary",
                                                                {tid!("status-system-mail-status-pending")}
                                                            }
                                                        }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{mail.recipient}" }
                                                    }
                                                    td {
                                                        div { class: "small text-break", "{mail.subject}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{worker}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{claimed_at_str}" }
                                                    }
                                                    td { class: "text-end",
                                                        div { class: "d-inline-flex gap-1 justify-content-end",
                                                            Button {
                                                                color: Color::Secondary,
                                                                outline: true,
                                                                size: Size::Sm,
                                                                onclick: move |_| {
                                                                    selected_mail.set(Some(mail_clone.clone()));
                                                                    show_detail_modal.set(true);
                                                                },
                                                                Icon { name: "eye", class: "me-1" }
                                                                {tid!("status-system-mail-view")}
                                                            }
                                                            if is_claimed {
                                                                Button {
                                                                    color: Color::Warning,
                                                                    outline: true,
                                                                    size: Size::Sm,
                                                                    onclick: move |_| {
                                                                        if let Err(e) = release(mail_id) {
                                                                            error!("release_system_mail failed: {e:?}");
                                                                        }
                                                                    },
                                                                    Icon { name: "arrow-counterclockwise", class: "me-1" }
                                                                    {tid!("status-system-mail-release")}
                                                                }
                                                            }
                                                            Button {
                                                                color: Color::Danger,
                                                                outline: true,
                                                                size: Size::Sm,
                                                                onclick: move |_| {
                                                                    if let Err(e) = complete(mail_id) {
                                                                        error!("complete_system_mail failed: {e:?}");
                                                                    }
                                                                },
                                                                Icon { name: "x-circle", class: "me-1" }
                                                                {tid!("status-system-mail-cancel")}
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
                    },
                }
            }
        }
        if let Some(target) = selected_mail() {
            Modal {
                show: show_detail_modal,
                title: tid!("status-system-mail-detail-title"),
                body: rsx! {
                    div { class: "mb-3",
                        strong { "{tid!(\"status-system-mail-id\")}: " }
                        code { "#{target.id}" }
                    }
                    div { class: "mb-3",
                        strong { "{tid!(\"status-system-mail-recipient\")}: " }
                        span { class: "text-muted ms-2", "{target.recipient}" }
                    }
                    div { class: "mb-3",
                        strong { "{tid!(\"status-system-mail-subject\")}: " }
                        span { class: "text-muted ms-2", "{target.subject}" }
                    }
                    if let Some(worker) = target.instance_id.as_deref() {
                        div { class: "mb-3",
                            strong { "{tid!(\"status-system-mail-worker\")}: " }
                            span { class: "badge bg-info text-white ms-2 me-2", "{worker}" }
                            if let Some(claimed_at) = target.claimed_at {
                                span { class: "text-muted small", "({claimed_at})" }
                            }
                        }
                    }
                    div { class: "mb-0",
                        strong { "{tid!(\"status-system-mail-body\")}:" }
                        pre {
                            class: "bg-light p-3 rounded mt-2 text-break border",
                            style: "white-space: pre-wrap; font-size: 0.875rem; max-height: 400px; overflow-y: auto;",
                            "{target.body_text}"
                        }
                    }
                },
                footer: rsx! {
                    Button {
                        color: Color::Secondary,
                        onclick: move |_| show_detail_modal.set(false),
                        {tid!("status-system-mail-close")}
                    }
                }
            }
        }
    }
}
