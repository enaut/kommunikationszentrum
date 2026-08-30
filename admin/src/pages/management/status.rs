use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;

use crate::module_bindings::dioxus::{
    use_connection_error, use_connection_state, use_subscription,
    use_table_sender_mail_delivery_claimed, use_table_sender_mail_delivery_done,
    use_table_sender_mail_delivery_events, use_table_sender_mail_delivery_pending,
    use_table_sender_mail_delivery_temporary_failed, use_table_visible_admin_identities,
    ConnectionState,
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
                        "Verwaltung · Status"
                    }
                }
            }
            ConnectionStatusCard { user_info: user_info.clone() }
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
        ConnectionState::Connected(id, _) => (
            Color::Success,
            "check-circle-fill",
            format!("Verbunden · Identity: {id}"),
        ),
        ConnectionState::Connecting => (
            Color::Info,
            "arrow-repeat",
            "Verbindung wird hergestellt…".to_string(),
        ),
        ConnectionState::Reconnecting { attempt, delay_ms } => (
            Color::Warning,
            "exclamation-triangle-fill",
            format!("Wiederverbinden… (Versuch {attempt}, {delay_ms} ms)"),
        ),
        ConnectionState::Error => (
            Color::Danger,
            "exclamation-circle-fill",
            "Verbindungsfehler".to_string(),
        ),
        ConnectionState::Disconnected => (
            Color::Secondary,
            "circle-fill",
            "Nicht verbunden".to_string(),
        ),
    };

    rsx! {
        Row { class: "mb-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-primary text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "plug-fill", class: "me-2" }
                            "SpacetimeDB Verbindung"
                        }
                    },
                    body: rsx! {
                        Alert {
                            color: alert_color,
                            class: "d-flex align-items-start",
                            role: "alert",
                            Icon { name: icon_name, class: "me-2 mt-1 flex-shrink-0" }
                            div { style: "overflow-x: auto; width: 100%;",
                                div { "{status_text}" }
                                if let Some(err) = conn_error() {
                                    div { class: "text-danger mt-1 small", "Fehler: {err}" }
                                }
                            }
                        }
                        Row { class: "text-center",
                            Col { md: ColumnSize::Span(4),
                                div { class: "border-end",
                                    h6 { class: "text-muted mb-1", "Mitgliedsnummer" }
                                    p { class: "h5 mb-0", "{user_info.mitgliedsnr}" }
                                }
                            }
                            Col { md: ColumnSize::Span(4),
                                div { class: "border-end",
                                    h6 { class: "text-muted mb-1", "E-Mail" }
                                    p { class: "h5 mb-0",
                                        if let Some(email) = &user_info.email {
                                            "{email}"
                                        } else {
                                            "–"
                                        }
                                    }
                                }
                            }
                            Col { md: ColumnSize::Span(4),
                                div {
                                    h6 { class: "text-muted mb-1", "ID Token" }
                                    p {
                                        style: "font-size: 0.55rem; word-break: break-all;",
                                        if let Some(token) = &user_info.id_token {
                                            "{token}"
                                        } else {
                                            "–"
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

#[component]
fn TemporaryFailedCard() -> Element {
    use_subscription(&["SELECT * FROM sender_mail_delivery_temporary_failed"]);
    let temporary_failed = use_table_sender_mail_delivery_temporary_failed();
    let cancel_retry = crate::module_bindings::dioxus::use_reducer_cancel_mail_delivery_retry();

    let mut rows = temporary_failed();
    rows.sort_by(|a, b| a.id.cmp(&b.id));

    rsx! {
        Row { class: "mt-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-warning text-dark",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "clock-history", class: "me-2" }
                            "mail_delivery_temporary_failed"
                            span { class: "badge bg-dark text-white ms-2",
                                "{rows.len()}"
                            }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0",
                                Icon { name: "inbox", class: "me-2" }
                                "Keine temporär fehlgeschlagenen Mail-Deliveries."
                            }
                        } else {
                            div { class: "table-responsive",
                                table { class: "table table-hover table-sm mb-0 align-middle",
                                    thead { class: "table-light",
                                        tr {
                                            th { "Delivery ID" }
                                            th { "Recipient" }
                                            th { "Retry at" }
                                            th { "Reason" }
                                            th { class: "text-end", "Action" }
                                        }
                                    }
                                    tbody {
                                        for failed in rows {
                                            {
                                                let delivery_id = failed.id.clone();
                                                let cancel = cancel_retry.clone();
                                                rsx! {
                                                    tr {
                                                        td { code { class: "small text-break", "{failed.id}" } }
                                                        td { small { class: "text-muted", "{failed.row.recipient_email}" } }
                                                        td { small { class: "text-muted", "{failed.next_attempt_at}" } }
                                                        td { div { class: "small text-break", "{failed.fail_reason}" } }
                                                        td { class: "text-end",
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
                                                                "Cancel"
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
                }
            }
        }
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
                            "mail_delivery_events"
                            span { class: "badge bg-white text-dark ms-2",
                                "{rows.len()}"
                            }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", "Keine Mail-Delivery-Events." }
                        } else {
                            div { class: "table-responsive",
                                table { class: "table table-sm table-hover mb-0",
                                    thead { class: "table-light",
                                        tr { th { "ID" } th { "Type" } th { "Attempt" } th { "Details" } }
                                    }
                                    tbody {
                                        for event in rows {
                                            tr {
                                                td { code { class: "small text-break", "{event.id}" } }
                                                td { small { class: "text-muted", "{event.event_type}" } }
                                                td { small { class: "text-muted", "{event.attempt_no}" } }
                                                td { div { class: "small text-break", "{event.details}" } }
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
}

#[component]
fn PendingCard() -> Element {
    use_subscription(&["SELECT * FROM sender_mail_delivery_pending"]);
    let pending_deliveries = use_table_sender_mail_delivery_pending();

    let mut rows = pending_deliveries();
    rows.sort_by(|a, b| a.id.cmp(&b.id));

    rsx! {
        Row { class: "mt-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-secondary text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "hourglass-split", class: "me-2" }
                            "mail_delivery_pending"
                            span { class: "badge bg-white text-secondary ms-2",
                                "{rows.len()}"
                            }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", "Keine offenen Deliveries." }
                        } else {
                            div { class: "table-responsive",
                                table { class: "table table-sm table-hover mb-0",
                                    thead { class: "table-light",
                                        tr { th { "ID" } th { "Recipient" } th { "Ingress" } }
                                    }
                                    tbody {
                                        for row in rows {
                                            tr {
                                                td { code { class: "small text-break", "{row.id}" } }
                                                td { small { class: "text-muted", "{row.row.recipient_email}" } }
                                                td { small { class: "text-muted", "{row.ingress_id}" } }
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
}

#[component]
fn ClaimedCard() -> Element {
    use_subscription(&["SELECT * FROM sender_mail_delivery_claimed"]);
    let claimed_deliveries = use_table_sender_mail_delivery_claimed();

    let mut rows = claimed_deliveries();
    rows.sort_by(|a, b| a.id.cmp(&b.id));

    rsx! {
        Row { class: "mt-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-info text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "person-workspace", class: "me-2" }
                            "mail_delivery_claimed"
                            span { class: "badge bg-white text-info ms-2",
                                "{rows.len()}"
                            }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", "Keine geclaimten Deliveries." }
                        } else {
                            div { class: "table-responsive",
                                table { class: "table table-sm table-hover mb-0",
                                    thead { class: "table-light",
                                        tr { th { "ID" } th { "Worker" } th { "Lease" } th { "Recipient" } }
                                    }
                                    tbody {
                                        for row in rows {
                                            tr {
                                                td { code { class: "small text-break", "{row.id}" } }
                                                td { small { class: "text-muted", "{row.worker}" } }
                                                td { small { class: "text-muted", "{row.lease_expires_at}" } }
                                                td { small { class: "text-muted", "{row.row.recipient_email}" } }
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
}

#[component]
fn DoneCard() -> Element {
    use_subscription(&["SELECT * FROM sender_mail_delivery_done"]);
    let done_deliveries = use_table_sender_mail_delivery_done();

    let mut rows = done_deliveries();
    rows.sort_by(|a, b| a.id.cmp(&b.id));

    rsx! {
        Row { class: "mt-4",
            Col { xs: ColumnSize::Span(12),
                Card {
                    class: "shadow-sm",
                    header_class: "bg-success text-white",
                    header: rsx! {
                        h5 { class: "card-title mb-0",
                            Icon { name: "check-circle", class: "me-2" }
                            "mail_delivery_done"
                            span { class: "badge bg-white text-success ms-2",
                                "{rows.len()}"
                            }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", "Keine abgeschlossenen Deliveries." }
                        } else {
                            div { class: "table-responsive",
                                table { class: "table table-sm table-hover mb-0",
                                    thead { class: "table-light",
                                        tr { th { "ID" } th { "Status" } th { "Recipient" } th { "Reason" } }
                                    }
                                    tbody {
                                        for row in rows {
                                            {
                                                let is_failed = row.final_state == "failed";
                                                let status_icon = if is_failed { "x-circle-fill" } else { "check-circle-fill" };
                                                let status_color = if is_failed { "text-danger" } else { "text-success" };
                                                let status_text = if is_failed { "failed" } else { "sent" };
                                                let reason = if let Some(ref err) = row.row.last_error { err.clone() } else { "—".to_string() };
                                                rsx! {
                                                    tr {
                                                        td { code { class: "small text-break", "{row.id}" } }
                                                        td {
                                                            span { class: "d-inline-flex align-items-center gap-2",
                                                                Icon { name: status_icon, class: format!("{status_color} fw-bold") }
                                                                small { class: "text-muted", "{status_text}" }
                                                            }
                                                        }
                                                        td { small { class: "text-muted", "{row.row.recipient_email}" } }
                                                        td { small { class: "text-break text-muted", "{reason}" } }
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
        }
    }
}
