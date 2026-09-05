use ::dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use dioxus_bootstrap_css::prelude::*;
use dioxus_i18n::tid;

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
                        "{tid!(\"management-status-title\") }"
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
        ConnectionState::Connected(id, _) => (Color::Primary, "check-circle-fill", format!("{id}")),
        ConnectionState::Connecting => (
            Color::Info,
            "arrow-repeat",
            tid!("status-connection-connecting"),
        ),
        ConnectionState::Reconnecting { attempt, delay_ms } => (
            Color::Warning,
            "exclamation-triangle-fill",
            format!(
                "{} ({attempt}, {delay_ms} ms)",
                tid!("status-connection-reconnecting")
            ),
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
                            "{tid!(\"status-connection-title\") }"
                            Icon { name: icon_name, class: "mx-3" }
                        }
                    },
                    body: rsx! {
                        Row { class: "text-center",
                            Col { md: ColumnSize::Span(4),
                                div { class: "border-end",
                                    h6 { class: "text-muted mb-1", "{tid!(\"status-identity\")}" }
                                    p { class: "h5 mb-0", "{status_text}" }
                                    if let Some(err) = conn_error() {
                                        div { class: "text-danger mt-1 small", "{tid!(\"status-error-label\")}: {err}" }
                                    }
                                }
                            }
                            Col { md: ColumnSize::Span(4),
                                div { class: "border-end",
                                    h6 { class: "text-muted mb-1", "{tid!(\"status-member-number\")}" }
                                    p { class: "h5 mb-0", "{user_info.mitgliedsnr}" }
                                }
                            }
                            Col { md: ColumnSize::Span(4),
                                div { class: "border-end",
                                    h6 { class: "text-muted mb-1", "{tid!(\"status-email\")}" }
                                    p { class: "h5 mb-0",
                                        if let Some(email) = &user_info.email {
                                            "{email}"
                                        } else {
                                            "{tid!(\"status-email-empty\") }"
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
                            "{tid!(\"status-temporary-failed-title\") }"
                            span { class: "badge bg-dark text-white ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0",
                                Icon { name: "inbox", class: "me-2" }
                                "{tid!(\"status-temporary-failed-empty\") }"
                            }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0 align-middle",
                                thead { class: "table-light",
                                    tr {
                                        th { "{tid!(\"status-temporary-failed-delivery-id\")}" }
                                        th { "{tid!(\"status-temporary-failed-recipient\")}" }
                                        th { "{tid!(\"status-temporary-failed-retry-at\")}" }
                                        th { "{tid!(\"status-temporary-failed-reason\")}" }
                                        th { class: "text-end", "{tid!(\"status-temporary-failed-action\")}" }
                                    }
                                }
                                tbody {
                                    for failed in rows {
                                        {
                                            let delivery_id = failed.id.clone();
                                            let cancel = cancel_retry.clone();
                                            rsx! {
                                                tr {
                                                    td {
                                                        code { class: "small text-break", "{failed.id}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{failed.row.recipient_email}" }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{failed.next_attempt_at}" }
                                                    }
                                                    td {
                                                        div { class: "small text-break", "{failed.fail_reason}" }
                                                    }
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
                                                            "{tid!(\"status-temporary-failed-cancel\") }"
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
                            "{tid!(\"status-delivery-events-title\") }"
                            span { class: "badge bg-white text-dark ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", "{tid!(\"status-delivery-events-empty\") }" }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0",
                                thead { class: "table-light",
                                    tr {
                                        th { "{tid!(\"status-delivery-events-id\")}" }
                                        th { "{tid!(\"status-delivery-events-type\")}" }
                                        th { "{tid!(\"status-delivery-events-attempt\")}" }
                                        th { "{tid!(\"status-delivery-events-details\")}" }
                                    }
                                }
                                tbody {
                                    for event in rows {
                                        tr {
                                            td {
                                                code { class: "small text-break", "{event.id}" }
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
                            "{tid!(\"status-pending-title\") }"
                            span { class: "badge bg-white text-secondary ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", "{tid!(\"status-pending-empty\") }" }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0",
                                thead { class: "table-light",
                                    tr {
                                        th { "{tid!(\"status-pending-id\")}" }
                                        th { "{tid!(\"status-pending-recipient\")}" }
                                        th { "{tid!(\"status-pending-ingress\")}" }
                                    }
                                }
                                tbody {
                                    for row in rows {
                                        tr {
                                            td {
                                                code { class: "small text-break", "{row.id}" }
                                            }
                                            td {
                                                small { class: "text-muted", "{row.row.recipient_email}" }
                                            }
                                            td {
                                                small { class: "text-muted", "{row.ingress_id}" }
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
                            "{tid!(\"status-claimed-title\") }"
                            span { class: "badge bg-white text-info ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", "{tid!(\"status-claimed-empty\") }" }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0",
                                thead { class: "table-light",
                                    tr {
                                        th { "{tid!(\"status-claimed-id\")}" }
                                        th { "{tid!(\"status-claimed-worker\")}" }
                                        th { "{tid!(\"status-claimed-lease\")}" }
                                        th { "{tid!(\"status-claimed-recipient\")}" }
                                    }
                                }
                                tbody {
                                    for row in rows {
                                        tr {
                                            td {
                                                code { class: "small text-break", "{row.id}" }
                                            }
                                            td {
                                                small { class: "text-muted", "{row.worker}" }
                                            }
                                            td {
                                                small { class: "text-muted", "{row.lease_expires_at}" }
                                            }
                                            td {
                                                small { class: "text-muted", "{row.row.recipient_email}" }
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
                            "{tid!(\"status-done-title\") }"
                            span { class: "badge bg-white text-success ms-2", "{rows.len()}" }
                        }
                    },
                    body: rsx! {
                        if rows.is_empty() {
                            p { class: "text-muted mb-0", "{tid!(\"status-done-empty\") }" }
                        } else {
                            Table {
                                hover: true,
                                size: Size::Sm,
                                responsive: true,
                                class: "mb-0",
                                thead { class: "table-light",
                                    tr {
                                        th { "{tid!(\"status-done-id\")}" }
                                        th { "{tid!(\"status-done-status\")}" }
                                        th { "{tid!(\"status-done-recipient\")}" }
                                        th { "{tid!(\"status-done-reason\")}" }
                                    }
                                }
                                tbody {
                                    for row in rows {
                                        {
                                            let is_failed = row.final_state == "failed";
                                            let status_icon = if is_failed { "x-circle-fill" } else { "check-circle-fill" };
                                            let status_color = if is_failed { "text-danger" } else { "text-success" };
                                            let status_text = if is_failed { tid!("status-done-failed") } else { tid!("status-done-sent") };
                                            let reason = if let Some(ref err) = row.row.last_error {
                                                err.clone()
                                            } else {
                                                tid!("general-none")
                                            };
                                            rsx! {
                                                tr {
                                                    td {
                                                        code { class: "small text-break", "{row.id}" }
                                                    }
                                                    td {
                                                        span { class: "d-inline-flex align-items-center gap-2",
                                                            Icon { name: status_icon, class: format!("{status_color} fw-bold") }
                                                            small { class: "text-muted", "{status_text}" }
                                                        }
                                                    }
                                                    td {
                                                        small { class: "text-muted", "{row.row.recipient_email}" }
                                                    }
                                                    td {
                                                        small { class: "text-break text-muted", "{reason}" }
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
