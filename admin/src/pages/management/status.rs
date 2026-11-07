use ::dioxus::prelude::*;
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;
use std::collections::BTreeMap;

use crate::i18n::format_timestamp;
use crate::module_bindings::dioxus::{
    ConnectionState, use_connection_error, use_connection_state,
    use_reducer_retry_failed_jmap_delivery_then, use_subscription,
    use_table_sender_jmap_mail_deliveries, use_table_sender_jmap_mail_delivery_events,
};
use crate::module_bindings::{JmapDeliveryState, JmapMailDelivery, JmapMailDeliveryEvent};
use crate::oauth::UserInfo;

#[component]
pub fn ManagementStatusPage(user_info: UserInfo) -> Element {
    use_subscription(&[
        "SELECT * FROM sender_jmap_mail_deliveries",
        "SELECT * FROM sender_jmap_mail_delivery_events",
        "SELECT * FROM visible_admin_identities",
    ]);
    let deliveries = use_table_sender_jmap_mail_deliveries();
    let events = use_table_sender_jmap_mail_delivery_events();
    let (retry_delivery, retry_result) = use_reducer_retry_failed_jmap_delivery_then();
    let mut retrying_delivery = use_signal(|| None::<String>);

    use_effect(move || {
        if retry_result().is_some() {
            retrying_delivery.set(None);
        }
    });

    let delivery_rows = deliveries();
    let event_rows = events();
    let counts = DeliveryCounts::from_rows(&delivery_rows);

    let mut ingress_groups: BTreeMap<String, Vec<JmapMailDelivery>> = BTreeMap::new();
    for delivery in delivery_rows.iter() {
        if let Some(ingress_id) = delivery.ingress_id.as_ref() {
            ingress_groups
                .entry(ingress_id.clone())
                .or_default()
                .push(delivery.clone());
        }
    }

    let mut sorted_deliveries = delivery_rows.clone();
    sorted_deliveries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let mut sorted_events = event_rows.clone();
    sorted_events.sort_by(|a, b| b.occurred_at.cmp(&a.occurred_at));
    let visible_events: Vec<_> = sorted_events.into_iter().take(100).collect();

    rsx! {
        Container { fluid: true, class: "mt-4 mb-5",
            Row { class: "mb-3",
                Col {
                    h2 { class: "mb-0",
                        Icon { name: "activity", class: "me-2" }
                        {tid!("management-status-title")}
                    }
                }
            }
            ConnectionStatusCard { user_info }

            Row { class: "mt-4 mb-3",
                Col {
                    h4 { class: "text-secondary d-flex align-items-center mb-0",
                        Icon { name: "send-check", class: "me-2" }
                        {tid!("status-jmap-title")}
                    }
                }
            }
            Row { class: "g-3 mb-4",
                SummaryCard {
                    label: tid!("status-jmap-queued"),
                    count: counts.queued,
                    color: Color::Secondary,
                    icon: "hourglass-split",
                }
                SummaryCard {
                    label: tid!("status-jmap-importing"),
                    count: counts.importing,
                    color: Color::Info,
                    icon: "cloud-arrow-up",
                }
                SummaryCard {
                    label: tid!("status-jmap-submitting"),
                    count: counts.submitting,
                    color: Color::Warning,
                    icon: "arrow-repeat",
                }
                SummaryCard {
                    label: tid!("status-jmap-sent"),
                    count: counts.sent,
                    color: Color::Success,
                    icon: "check-circle",
                }
                SummaryCard {
                    label: tid!("status-jmap-retrying"),
                    count: counts.retrying,
                    color: Color::Primary,
                    icon: "clock-history",
                }
                SummaryCard {
                    label: tid!("status-jmap-failed"),
                    count: counts.failed,
                    color: Color::Danger,
                    icon: "exclamation-octagon",
                }
            }

            if let Some(result) = retry_result() {
                {
                    match result {
                        Ok(()) => rsx! {
                            Alert { color: Color::Success, class: "mb-3", {tid!("status-jmap-retry-success")} }
                        },
                        Err(message) => rsx! {
                            Alert { color: Color::Danger, class: "mb-3", {tid!("status-jmap-retry-error", error : message)} }
                        },
                    }
                }
            }

            IngressProgressCard { groups: ingress_groups }
            DeliveriesCard {
                rows: sorted_deliveries,
                events: event_rows.clone(),
                retry_delivery: Callback::new(move |delivery_id| retry_delivery(delivery_id)),
                retrying_delivery,
            }
            DeliveryEventsCard { rows: visible_events }
        }
    }
}

#[derive(Default)]
struct DeliveryCounts {
    queued: usize,
    importing: usize,
    submitting: usize,
    sent: usize,
    retrying: usize,
    failed: usize,
}

impl DeliveryCounts {
    fn from_rows(rows: &[JmapMailDelivery]) -> Self {
        let mut counts = Self::default();
        for row in rows {
            match row.state {
                JmapDeliveryState::Pending if row.attempt_count == 0 => counts.queued += 1,
                JmapDeliveryState::Pending => counts.retrying += 1,
                JmapDeliveryState::Importing | JmapDeliveryState::Imported => {
                    counts.importing += 1;
                }
                JmapDeliveryState::Submitting | JmapDeliveryState::SubmissionUnknown => {
                    counts.submitting += 1;
                }
                JmapDeliveryState::Sent => counts.sent += 1,
                JmapDeliveryState::Failed => counts.failed += 1,
            }
        }
        counts
    }
}

#[component]
fn ConnectionStatusCard(user_info: UserInfo) -> Element {
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
                                            {tid!("status-connection-error-details", error : err)}
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
                                div {
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
fn SummaryCard(label: String, count: usize, color: Color, icon: &'static str) -> Element {
    rsx! {
        Col {
            xs: ColumnSize::Span(6),
            md: ColumnSize::Span(4),
            xl: ColumnSize::Span(2),
            Card {
                class: "shadow-sm h-100 border-{color}",
                body: rsx! {
                    div { class: "d-flex align-items-center justify-content-between gap-2",
                        div {
                            div { class: "text-muted small", "{label}" }
                            div { class: "fs-3 fw-semibold", "{count}" }
                        }
                        Icon { name: icon, class: "text-primary fs-3" }
                    }
                },
            }
        }
    }
}

#[component]
fn IngressProgressCard(groups: BTreeMap<String, Vec<JmapMailDelivery>>) -> Element {
    let mut rows: Vec<_> = groups.into_iter().collect();
    rows.sort_by(|(_, a), (_, b)| {
        let a_updated = a.iter().map(|d| d.updated_at).max();
        let b_updated = b.iter().map(|d| d.updated_at).max();
        b_updated.cmp(&a_updated)
    });

    rsx! {
        Row { class: "mb-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "d-flex justify-content-between align-items-center",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "inboxes-fill", class: "me-2" }
                            {tid!("status-jmap-ingress-title")}
                        }
                        span { class: "badge bg-secondary", "{rows.len()}" }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", {tid!("status-jmap-ingress-empty")} }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0 align-middle",
                                thead { class: "table-light",
                                    tr {
                                        th { {tid!("status-jmap-ingress-id")} }
                                        th { {tid!("status-jmap-subject")} }
                                        th { {tid!("status-jmap-topic")} }
                                        th { {tid!("status-jmap-progress")} }
                                        th { {tid!("status-last-updated")} }
                                    }
                                }
                                tbody {
                                    for (ingress_id, deliveries) in rows {
                                        {
                                            let first = deliveries.first();
                                            let subject = first.map(|d| d.subject.as_str()).unwrap_or("–");
                                            let topic = first
                                                .and_then(|d| d.topic_id)
                                                .map(|id| id.to_string())
                                                .unwrap_or_else(|| "–".to_string());
                                            let sent = deliveries
                                                .iter()
                                                .filter(|d| d.state == JmapDeliveryState::Sent)
                                                .count();
                                            let failed = deliveries
                                                .iter()
                                                .filter(|d| d.state == JmapDeliveryState::Failed)
                                                .count();
                                            let total = deliveries.len();
                                            let latest = deliveries.iter().map(|d| d.updated_at).max();
                                            let updated = latest.map(format_timestamp).unwrap_or_else(|| "–".to_string());
                                            let progress_percent = if total > 0 { sent * 100 / total } else { 0 };
                                            rsx! {
                                                tr {
                                                    td {
                                                        code { class: "small text-break", "{ingress_id}" }
                                                    }
                                                    td {
                                                        div { class: "small text-break", "{subject}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{topic}" }
                                                    }
                                                    td {
                                                        div { class: "d-flex align-items-center gap-2",
                                                            div {
                                                                class: "progress flex-grow-1",
                                                                style: "min-width: 90px; height: 0.75rem;",
                                                                div {
                                                                    class: "progress-bar bg-success",
                                                                    role: "progressbar",
                                                                    style: "width: {progress_percent}%",
                                                                    "aria-valuenow": "{progress_percent}",
                                                                    "aria-valuemin": "0",
                                                                    "aria-valuemax": "100",
                                                                }
                                                            }
                                                            small { class: "text-nowrap", "{sent} / {total}" }
                                                            if failed > 0 {
                                                                Badge { color: Color::Danger, {tid!("status-jmap-failed-count", count : failed)} }
                                                            }
                                                        }
                                                    }
                                                    td {
                                                        small { class: "text-muted text-nowrap", "{updated}" }
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

#[component]
fn DeliveriesCard(
    rows: Vec<JmapMailDelivery>,
    events: Vec<JmapMailDeliveryEvent>,
    retry_delivery: Callback<String>,
    retrying_delivery: Signal<Option<String>>,
) -> Element {
    let mut selected_delivery = use_signal(|| None::<String>);
    let mut show_detail = use_signal(|| false);

    rsx! {
        Row { class: "mb-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "d-flex justify-content-between align-items-center",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "envelope-paper-fill", class: "me-2" }
                            {tid!("status-jmap-deliveries-title")}
                        }
                        span { class: "badge bg-secondary", "{rows.len()}" }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", {tid!("status-jmap-deliveries-empty")} }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0 align-middle",
                                thead { class: "table-light",
                                    tr {
                                        th { {tid!("status-jmap-delivery-id")} }
                                        th { {tid!("status-jmap-state")} }
                                        th { {tid!("status-jmap-recipient")} }
                                        th { {tid!("status-jmap-subject")} }
                                        th { {tid!("status-jmap-attempts")} }
                                        th { {tid!("status-last-updated")} }
                                        th { class: "text-end", {tid!("status-jmap-actions")} }
                                    }
                                }
                                tbody {
                                    for delivery in &rows {
                                        {
                                            let id = delivery.delivery_id.clone();
                                            let id_for_detail = id.clone();
                                            let id_for_retry = id.clone();
                                            let retry = retry_delivery.clone();
                                            let retry_in_progress = retrying_delivery().is_some();
                                            let is_retrying = retrying_delivery().as_deref() == Some(id.as_str());
                                            let (color, label) = state_label(delivery.state);
                                            let ingress = delivery.ingress_id.as_deref().unwrap_or("–");
                                            rsx! {
                                                tr {
                                                    td {
                                                        code { class: "small text-break", "{delivery.delivery_id}" }
                                                    }
                                                    td {
                                                        Badge { color, "{label}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{delivery.recipient_email}" }
                                                    }
                                                    td {
                                                        div { class: "small text-break", "{delivery.subject}" }
                                                        if delivery.ingress_id.is_some() {
                                                            small { class: "text-muted", {tid!("status-jmap-ingress-prefix", id : ingress)} }
                                                        } else {
                                                            small { class: "text-muted", {tid!("status-jmap-system-mail")} }
                                                        }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{delivery.attempt_count}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted text-nowrap", "{format_timestamp(delivery.updated_at)}" }
                                                    }
                                                    td { class: "text-end",
                                                        div { class: "d-inline-flex gap-1 justify-content-end",
                                                            Button {
                                                                color: Color::Secondary,
                                                                outline: true,
                                                                size: Size::Sm,
                                                                onclick: move |_| {
                                                                    selected_delivery.set(Some(id_for_detail.clone()));
                                                                    show_detail.set(true);
                                                                },
                                                                Icon { name: "eye", class: "me-1" }
                                                                {tid!("status-jmap-view")}
                                                            }
                                                            if delivery.state == JmapDeliveryState::Failed {
                                                                Button {
                                                                    color: Color::Warning,
                                                                    outline: true,
                                                                    size: Size::Sm,
                                                                    disabled: retry_in_progress,
                                                                    onclick: move |_| {
                                                                        retrying_delivery.set(Some(id_for_retry.clone()));
                                                                        retry.call(id_for_retry.clone());
                                                                    },
                                                                    if is_retrying {
                                                                        Spinner { color: Color::Warning, class: "me-1" }
                                                                    } else {
                                                                        Icon { name: "arrow-clockwise", class: "me-1" }
                                                                    }
                                                                    {tid!("status-jmap-retry")}
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
        DeliveryDetailModal {
            delivery: selected_delivery()
                .and_then(|id| rows.iter().find(|row| row.delivery_id == id).cloned()),
            events,
            show: show_detail,
        }
    }
}

#[component]
fn DeliveryDetailModal(
    delivery: Option<JmapMailDelivery>,
    events: Vec<JmapMailDeliveryEvent>,
    show: Signal<bool>,
) -> Element {
    if let Some(delivery) = delivery {
        let mut delivery_events: Vec<_> = events
            .into_iter()
            .filter(|event| event.delivery_id == delivery.delivery_id)
            .collect();
        delivery_events.sort_by(|a, b| b.occurred_at.cmp(&a.occurred_at));
        let (color, label) = state_label(delivery.state);
        rsx! {
            Modal {
                show,
                title: tid!("status-jmap-detail-title"),
                body: rsx! {
                    div { class: "mb-3",
                        strong {
                            {tid!("status-jmap-delivery-id")}
                            ": "
                        }
                        code { class: "text-break", "{delivery.delivery_id}" }
                    }
                    div { class: "mb-3",
                        strong {
                            {tid!("status-jmap-state")}
                            ": "
                        }
                        Badge { color, "{label}" }
                    }
                    div { class: "mb-3",
                        strong {
                            {tid!("status-jmap-recipient")}
                            ": "
                        }
                        span { class: "text-muted ms-2", "{delivery.recipient_email}" }
                    }
                    div { class: "mb-3",
                        strong {
                            {tid!("status-jmap-subject")}
                            ": "
                        }
                        span { class: "text-muted ms-2", "{delivery.subject}" }
                    }
                    if let Some(ingress_id) = &delivery.ingress_id {
                        div { class: "mb-3",
                            strong {
                                {tid!("status-jmap-ingress-id")}
                                ": "
                            }
                            code { class: "ms-2", "{ingress_id}" }
                        }
                    }
                    if let Some(system_mail_id) = delivery.system_mail_id {
                        div { class: "mb-3",
                            strong {
                                {tid!("status-jmap-system-mail-id")}
                                ": "
                            }
                            code { class: "ms-2", "{system_mail_id}" }
                        }
                    }
                    div { class: "mb-3",
                        strong {
                            {tid!("status-jmap-topic")}
                            ": "
                        }
                        span { class: "text-muted ms-2",
                            {delivery.topic_id.map(|id| id.to_string()).unwrap_or_else(|| "–".to_string())}
                        }
                    }
                    div { class: "mb-3",
                        strong {
                            {tid!("status-jmap-attempts")}
                            ": "
                        }
                        span { class: "text-muted ms-2", "{delivery.attempt_count}" }
                    }
                    div { class: "mb-3",
                        strong {
                            {tid!("status-jmap-next-attempt")}
                            ": "
                        }
                        span { class: "text-muted ms-2", "{format_timestamp(delivery.next_attempt_at)}" }
                    }
                    if let Some(email_id) = &delivery.jmap_email_id {
                        div { class: "mb-3",
                            strong {
                                {tid!("status-jmap-email-id")}
                                ": "
                            }
                            code { class: "ms-2 text-break", "{email_id}" }
                        }
                    }
                    if let Some(submission_id) = &delivery.jmap_submission_id {
                        div { class: "mb-3",
                            strong {
                                {tid!("status-jmap-submission-id")}
                                ": "
                            }
                            code { class: "ms-2 text-break", "{submission_id}" }
                        }
                    }
                    if let Some(error_message) = &delivery.last_error {
                        div { class: "alert alert-danger py-2 px-3 small text-break", "{error_message}" }
                    }
                    div { class: "small text-muted mb-3",
                        strong {
                            {tid!("status-jmap-created")}
                            ": "
                        }
                        "{format_timestamp(delivery.created_at)} · "
                        strong {
                            {tid!("status-last-updated")}
                            ": "
                        }
                        "{format_timestamp(delivery.updated_at)}"
                    }
                    h6 { class: "mt-4", {tid!("status-jmap-events-title")} }
                    if delivery_events.is_empty() {
                        p { class: "text-muted small", {tid!("status-jmap-events-empty")} }
                    } else {
                        div { class: "list-group list-group-flush",
                            for event in delivery_events {
                                div { class: "list-group-item px-0 py-2",
                                    div { class: "d-flex justify-content-between gap-2",
                                        strong { class: "small", "{event.event_type}" }
                                        small { class: "text-muted text-nowrap", "{format_timestamp(event.occurred_at)}" }
                                    }
                                    div { class: "small text-muted",
                                        {tid!("status-jmap-attempt-number", attempt : event.attempt_no)}
                                    }
                                    if !event.details.is_empty() {
                                        div { class: "small text-break mt-1", "{event.details}" }
                                    }
                                    if let Some(email_id) = event.jmap_email_id {
                                        div { class: "small text-muted text-break",
                                            {tid!("status-jmap-event-email-id", id : email_id)}
                                        }
                                    }
                                    if let Some(submission_id) = event.jmap_submission_id {
                                        div { class: "small text-muted text-break",
                                            {tid!("status-jmap-event-submission-id", id : submission_id)}
                                        }
                                    }
                                    if let Some(worker) = event.worker_identity {
                                        div { class: "small text-muted text-break",
                                            {tid!("status-jmap-event-worker", worker : worker.to_string())}
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                footer: rsx! {
                    Button { color: Color::Secondary, onclick: move |_| show.set(false), {tid!("general-cancel")} }
                },
            }
        }
    } else {
        rsx! {}
    }
}

#[component]
fn DeliveryEventsCard(rows: Vec<JmapMailDeliveryEvent>) -> Element {
    let rows_limited = rows.len() == 100;
    rsx! {
        Row { class: "mb-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "d-flex justify-content-between align-items-center",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "journal-text", class: "me-2" }
                            {tid!("status-jmap-events-title")}
                        }
                        span { class: "badge bg-secondary", "{rows.len()}" }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", {tid!("status-jmap-events-empty")} }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0 align-middle",
                                thead { class: "table-light",
                                    tr {
                                        th { {tid!("status-jmap-delivery-id")} }
                                        th { {tid!("status-jmap-event-type")} }
                                        th { {tid!("status-jmap-attempts")} }
                                        th { {tid!("status-jmap-event-time")} }
                                        th { {tid!("status-jmap-event-worker-label")} }
                                        th { {tid!("status-jmap-details")} }
                                    }
                                }
                                tbody {
                                    for event in rows {
                                        tr {
                                            td {
                                                code { class: "small text-break", "{event.delivery_id}" }
                                            }
                                            td {
                                                small { "{event.event_type}" }
                                            }
                                            td {
                                                small { class: "text-muted", "{event.attempt_no}" }
                                            }
                                            td {
                                                small { class: "text-muted text-nowrap", "{format_timestamp(event.occurred_at)}" }
                                            }
                                            td {
                                                small { class: "text-muted text-break",
                                                    {
                                                        event
                                                            .worker_identity
                                                            .map(|identity| identity.to_string())
                                                            .unwrap_or_else(|| "–".to_string())
                                                    }
                                                }
                                            }
                                            td {
                                                div { class: "small text-break", "{event.details}" }
                                            }
                                        }
                                    }
                                }
                            }
                            if rows_limited {
                                p { class: "text-muted small mt-2 mb-0", {tid!("status-jmap-events-limited")} }
                            }
                        }
                    },
                }
            }
        }
    }
}

fn state_label(state: JmapDeliveryState) -> (Color, String) {
    match state {
        JmapDeliveryState::Pending => (Color::Secondary, tid!("status-jmap-state-queued")),
        JmapDeliveryState::Importing => (Color::Info, tid!("status-jmap-state-importing")),
        JmapDeliveryState::Imported => (Color::Info, tid!("status-jmap-state-imported")),
        JmapDeliveryState::Submitting => (Color::Warning, tid!("status-jmap-state-submitting")),
        JmapDeliveryState::SubmissionUnknown => (Color::Warning, tid!("status-jmap-state-unknown")),
        JmapDeliveryState::Sent => (Color::Success, tid!("status-jmap-state-sent")),
        JmapDeliveryState::Failed => (Color::Danger, tid!("status-jmap-state-failed")),
    }
}
