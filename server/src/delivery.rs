use spacetimedb::{Identity, Query, ReducerContext, Table, TimeDuration, Timestamp, ViewContext};

pub const MAIL_INGRESS_PENDING: &str = "pending";
pub const MAIL_INGRESS_PROCESSING: &str = "processing";
pub const MAIL_INGRESS_RETRY_SCHEDULED: &str = "retry_scheduled";
pub const MAIL_INGRESS_COMPLETED: &str = "completed";
pub const MAIL_INGRESS_FAILED: &str = "failed";

pub const MAIL_DELIVERY_QUEUED: &str = "queued";
pub const MAIL_DELIVERY_SENDING: &str = "sending";
pub const MAIL_DELIVERY_RETRY_SCHEDULED: &str = "retry_scheduled";
pub const MAIL_DELIVERY_SENT: &str = "sent";
pub const MAIL_DELIVERY_FAILED: &str = "failed";
pub const MAIL_DELIVERY_BOUNCED: &str = "bounced";

fn ingress_lease_duration() -> TimeDuration {
    TimeDuration::from_micros(10 * 60 * 1_000_000)
}

fn delivery_lease_duration() -> TimeDuration {
    TimeDuration::from_micros(5 * 60 * 1_000_000)
}

fn delivery_retry_backoff(attempt_count: u32) -> TimeDuration {
    match attempt_count {
        1 => TimeDuration::from_micros(30 * 1_000_000),
        2 => TimeDuration::from_micros(2 * 60 * 1_000_000),
        3 => TimeDuration::from_micros(10 * 60 * 1_000_000),
        4 => TimeDuration::from_micros(30 * 60 * 1_000_000),
        5 => TimeDuration::from_micros(60 * 60 * 1_000_000),
        _ => TimeDuration::from_micros(12 * 60 * 60 * 1_000_000),
    }
}

fn ingress_retry_backoff(attempt_count: u32) -> TimeDuration {
    match attempt_count {
        1 => TimeDuration::from_micros(30 * 1_000_000),
        2 => TimeDuration::from_micros(2 * 60 * 1_000_000),
        3 => TimeDuration::from_micros(10 * 60 * 1_000_000),
        _ => TimeDuration::from_micros(30 * 60 * 1_000_000),
    }
}

#[derive(Clone)]
#[spacetimedb::table(accessor = mail_ingress, public)]
pub struct MailIngress {
    #[primary_key]
    pub id: String,
    #[index(btree)]
    pub queue_id: String,
    #[index(btree)]
    pub category_id: u64,
    #[index(btree)]
    pub state: String,
    #[index(btree)]
    pub next_attempt_at: Timestamp,
    #[index(btree)]
    pub received_at: Timestamp,
    pub sender_account_id: Option<u64>,
    pub sender_email: String,
    pub category_email: String,
    pub subject: String,
    pub from_header: String,
    pub reply_to: Option<String>,
    pub date_header: Option<String>,
    pub message_id: Option<String>,
    pub cc_header: Option<String>,
    pub headers_raw: String,
    pub body_raw: String,
    pub message_size: u64,
    pub claim_owner: Option<Identity>,
    pub claim_expires_at: Timestamp,
    pub attempt_count: u32,
    pub recipient_count: u32,
    pub delivery_count: u32,
    pub failed_delivery_count: u32,
    pub last_error: Option<String>,
    pub completed_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Clone)]
#[spacetimedb::table(accessor = mail_deliveries, public)]
pub struct MailDelivery {
    #[primary_key]
    pub id: String,
    #[index(btree)]
    pub ingress_id: String,
    #[index(btree)]
    pub category_id: u64,
    #[index(btree)]
    pub subscription_id: u64,
    #[index(btree)]
    pub recipient_email: String,
    #[index(btree)]
    pub state: String,
    #[index(btree)]
    pub next_attempt_at: Timestamp,
    pub recipient_account_id: Option<u64>,
    pub list_email: String,
    pub list_name: String,
    pub original_sender_email: String,
    pub from_header: String,
    pub reply_to: String,
    pub subject: String,
    pub body_raw: String,
    pub headers_raw: String,
    pub raw_message: String,
    pub unsubscribe_token: String,
    pub claim_owner: Option<Identity>,
    pub claim_expires_at: Timestamp,
    pub attempt_count: u32,
    pub sent_at: Timestamp,
    pub last_error: Option<String>,
    pub smtp_status_code: Option<u16>,
    pub smtp_response: Option<String>,
    pub updated_at: Timestamp,
}

#[derive(Clone)]
#[spacetimedb::table(accessor = mail_delivery_events, public)]
pub struct MailDeliveryEvent {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub delivery_id: String,
    #[index(btree)]
    pub occurred_at: Timestamp,
    pub event_type: String,
    pub attempt_no: u32,
    pub smtp_status_code: Option<u16>,
    pub smtp_response: Option<String>,
    pub error_kind: Option<String>,
    pub details: String,
    pub worker_identity: Option<Identity>,
}

#[spacetimedb::view(accessor = sender_mail_ingress, public)]
pub fn sender_mail_ingress(ctx: &ViewContext) -> impl Query<MailIngress> {
    ctx.from.mail_ingress().r#filter(|_| true)
}

#[spacetimedb::view(accessor = sender_mail_deliveries, public)]
pub fn sender_mail_deliveries(ctx: &ViewContext) -> impl Query<MailDelivery> {
    ctx.from.mail_deliveries().r#filter(|_| true)
}

fn make_ingress_id(ctx: &ReducerContext, queue_id: &str, category_id: u64) -> String {
    let entropy = ctx.random::<u128>();
    format!("{queue_id}:{category_id}:{entropy:032x}")
}

pub(crate) fn make_delivery_id(
    ingress_id: &str,
    subscription_id: u64,
    recipient_email: &str,
) -> String {
    format!("{ingress_id}:{subscription_id}:{recipient_email}")
}

pub(crate) fn upsert_mail_ingress(
    ctx: &ReducerContext,
    queue_id: Option<String>,
    category_id: u64,
    category_email: String,
    sender_account_id: Option<u64>,
    sender_email: String,
    subject: String,
    from_header: String,
    reply_to: Option<String>,
    date_header: Option<String>,
    message_id: Option<String>,
    cc_header: Option<String>,
    headers_raw: String,
    body_raw: String,
    message_size: u64,
) -> String {
    let queue_id_value = queue_id.unwrap_or_default();
    let ingress_id = make_ingress_id(ctx, &queue_id_value, category_id);

    if ctx.db.mail_ingress().id().find(&ingress_id).is_none() {
        ctx.db.mail_ingress().insert(MailIngress {
            id: ingress_id.clone(),
            queue_id: queue_id_value,
            category_id,
            state: MAIL_INGRESS_PENDING.to_string(),
            next_attempt_at: ctx.timestamp,
            received_at: ctx.timestamp,
            sender_account_id,
            sender_email,
            category_email,
            subject,
            from_header,
            reply_to,
            date_header,
            message_id,
            cc_header,
            headers_raw,
            body_raw,
            message_size,
            claim_owner: None,
            claim_expires_at: Timestamp::UNIX_EPOCH,
            attempt_count: 0,
            recipient_count: 0,
            delivery_count: 0,
            failed_delivery_count: 0,
            last_error: None,
            completed_at: Timestamp::UNIX_EPOCH,
            updated_at: ctx.timestamp,
        });
    }

    ingress_id
}

fn set_ingress_claim(
    mut row: MailIngress,
    ctx: &ReducerContext,
    state: &str,
    lease: TimeDuration,
) -> MailIngress {
    row.state = state.to_string();
    row.claim_owner = Some(ctx.sender());
    row.claim_expires_at = ctx.timestamp + lease;
    row.attempt_count = row.attempt_count.saturating_add(1);
    row.next_attempt_at = ctx.timestamp;
    row.last_error = None;
    row.updated_at = ctx.timestamp;
    row
}

fn set_delivery_claim(
    mut row: MailDelivery,
    ctx: &ReducerContext,
    state: &str,
    lease: TimeDuration,
) -> MailDelivery {
    row.state = state.to_string();
    row.claim_owner = Some(ctx.sender());
    row.claim_expires_at = ctx.timestamp + lease;
    row.attempt_count = row.attempt_count.saturating_add(1);
    row.next_attempt_at = ctx.timestamp;
    row.last_error = None;
    row.updated_at = ctx.timestamp;
    row
}

fn claimable_ingress(row: &MailIngress, now: Timestamp) -> bool {
    matches!(
        row.state.as_str(),
        MAIL_INGRESS_PENDING | MAIL_INGRESS_RETRY_SCHEDULED
    ) && row.next_attempt_at <= now
        && (row.claim_owner.is_none() || row.claim_expires_at <= now)
}

fn claimable_delivery(row: &MailDelivery, now: Timestamp) -> bool {
    matches!(
        row.state.as_str(),
        MAIL_DELIVERY_QUEUED | MAIL_DELIVERY_RETRY_SCHEDULED
    ) && row.next_attempt_at <= now
        && (row.claim_owner.is_none() || row.claim_expires_at <= now)
}

#[spacetimedb::reducer]
pub fn claim_next_mail_ingress(ctx: &ReducerContext) -> Result<(), String> {
    let mut candidates: Vec<MailIngress> = ctx
        .db
        .mail_ingress()
        .iter()
        .filter(|row| claimable_ingress(row, ctx.timestamp))
        .collect();

    candidates.sort_by(|left, right| {
        left.next_attempt_at
            .cmp(&right.next_attempt_at)
            .then(left.received_at.cmp(&right.received_at))
            .then(left.id.cmp(&right.id))
    });

    let Some(row) = candidates.into_iter().next() else {
        return Ok(());
    };

    let claimed = set_ingress_claim(row, ctx, MAIL_INGRESS_PROCESSING, ingress_lease_duration());
    ctx.db.mail_ingress().id().update(claimed);
    Ok(())
}

#[spacetimedb::reducer]
pub fn complete_mail_ingress(
    ctx: &ReducerContext,
    ingress_id: String,
    delivery_count: u32,
    failed_delivery_count: u32,
) -> Result<(), String> {
    let Some(mut row) = ctx.db.mail_ingress().id().find(&ingress_id) else {
        return Err(format!("Mail ingress '{ingress_id}' not found"));
    };

    if row.claim_owner != Some(ctx.sender()) {
        return Err(format!(
            "Mail ingress '{ingress_id}' is not owned by {:?}",
            ctx.sender()
        ));
    }

    row.state = MAIL_INGRESS_COMPLETED.to_string();
    row.delivery_count = delivery_count;
    row.failed_delivery_count = failed_delivery_count;
    row.last_error = None;
    row.claim_owner = None;
    row.claim_expires_at = Timestamp::UNIX_EPOCH;
    row.completed_at = ctx.timestamp;
    row.updated_at = ctx.timestamp;
    ctx.db.mail_ingress().id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn retry_mail_ingress(
    ctx: &ReducerContext,
    ingress_id: String,
    error: String,
) -> Result<(), String> {
    let Some(mut row) = ctx.db.mail_ingress().id().find(&ingress_id) else {
        return Err(format!("Mail ingress '{ingress_id}' not found"));
    };

    if row.claim_owner != Some(ctx.sender()) {
        return Err(format!(
            "Mail ingress '{ingress_id}' is not owned by {:?}",
            ctx.sender()
        ));
    }

    row.last_error = Some(error.clone());
    row.claim_owner = None;
    row.claim_expires_at = Timestamp::UNIX_EPOCH;
    row.updated_at = ctx.timestamp;

    if row.attempt_count >= 5 {
        row.state = MAIL_INGRESS_FAILED.to_string();
        row.completed_at = ctx.timestamp;
    } else {
        row.state = MAIL_INGRESS_RETRY_SCHEDULED.to_string();
        row.next_attempt_at = ctx.timestamp + ingress_retry_backoff(row.attempt_count);
    }

    ctx.db.mail_ingress().id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn fail_mail_ingress(
    ctx: &ReducerContext,
    ingress_id: String,
    error: String,
) -> Result<(), String> {
    let Some(mut row) = ctx.db.mail_ingress().id().find(&ingress_id) else {
        return Err(format!("Mail ingress '{ingress_id}' not found"));
    };

    if row.claim_owner != Some(ctx.sender()) {
        return Err(format!(
            "Mail ingress '{ingress_id}' is not owned by {:?}",
            ctx.sender()
        ));
    }

    row.state = MAIL_INGRESS_FAILED.to_string();
    row.last_error = Some(error);
    row.claim_owner = None;
    row.claim_expires_at = Timestamp::UNIX_EPOCH;
    row.completed_at = ctx.timestamp;
    row.updated_at = ctx.timestamp;
    ctx.db.mail_ingress().id().update(row);
    Ok(())
}

pub(crate) fn upsert_mail_delivery(
    ctx: &ReducerContext,
    ingress: &MailIngress,
    subscription_id: u64,
    recipient_email: String,
    recipient_account_id: Option<u64>,
    list_email: String,
    list_name: String,
    original_sender_email: String,
    from_header: String,
    reply_to: String,
    subject: String,
    body_raw: String,
    headers_raw: String,
    raw_message: String,
    unsubscribe_token: String,
) -> String {
    let delivery_id = make_delivery_id(&ingress.id, subscription_id, &recipient_email);

    match ctx.db.mail_deliveries().id().find(&delivery_id) {
        Some(existing)
            if matches!(
                existing.state.as_str(),
                MAIL_DELIVERY_SENT | MAIL_DELIVERY_FAILED | MAIL_DELIVERY_BOUNCED
            ) =>
        {
            // Keep terminal deliveries immutable.
            delivery_id
        }
        Some(mut existing) => {
            existing.ingress_id = ingress.id.clone();
            existing.category_id = ingress.category_id;
            existing.subscription_id = subscription_id;
            existing.recipient_email = recipient_email;
            existing.recipient_account_id = recipient_account_id;
            existing.list_email = list_email;
            existing.list_name = list_name;
            existing.original_sender_email = original_sender_email;
            existing.from_header = from_header;
            existing.reply_to = reply_to;
            existing.subject = subject;
            existing.body_raw = body_raw;
            existing.headers_raw = headers_raw;
            existing.raw_message = raw_message;
            existing.unsubscribe_token = unsubscribe_token;
            existing.updated_at = ctx.timestamp;
            if existing.state == MAIL_DELIVERY_RETRY_SCHEDULED {
                existing.next_attempt_at = ctx.timestamp;
            }
            ctx.db.mail_deliveries().id().update(existing);
            delivery_id
        }
        None => {
            ctx.db.mail_deliveries().insert(MailDelivery {
                id: delivery_id.clone(),
                ingress_id: ingress.id.clone(),
                category_id: ingress.category_id,
                subscription_id,
                recipient_email,
                state: MAIL_DELIVERY_QUEUED.to_string(),
                next_attempt_at: ctx.timestamp,
                recipient_account_id,
                list_email,
                list_name,
                original_sender_email,
                from_header,
                reply_to,
                subject,
                body_raw,
                headers_raw,
                raw_message,
                unsubscribe_token,
                claim_owner: None,
                claim_expires_at: Timestamp::UNIX_EPOCH,
                attempt_count: 0,
                sent_at: Timestamp::UNIX_EPOCH,
                last_error: None,
                smtp_status_code: None,
                smtp_response: None,
                updated_at: ctx.timestamp,
            });
            ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
                id: 0,
                delivery_id: delivery_id.clone(),
                occurred_at: ctx.timestamp,
                event_type: MAIL_DELIVERY_QUEUED.to_string(),
                attempt_no: 0,
                smtp_status_code: None,
                smtp_response: None,
                error_kind: None,
                details: "Delivery queued for SMTP submission".to_string(),
                worker_identity: Some(ctx.sender()),
            });
            delivery_id
        }
    }
}

#[spacetimedb::reducer]
pub fn enqueue_mail_delivery(
    ctx: &ReducerContext,
    ingress_id: String,
    subscription_id: u64,
    recipient_email: String,
    recipient_account_id: Option<u64>,
    list_email: String,
    list_name: String,
    original_sender_email: String,
    from_header: String,
    reply_to: String,
    subject: String,
    body_raw: String,
    headers_raw: String,
    raw_message: String,
    unsubscribe_token: String,
) -> Result<(), String> {
    let Some(ingress) = ctx.db.mail_ingress().id().find(&ingress_id) else {
        return Err(format!("Mail ingress '{ingress_id}' not found"));
    };
    upsert_mail_delivery(
        ctx,
        &ingress,
        subscription_id,
        recipient_email,
        recipient_account_id,
        list_email,
        list_name,
        original_sender_email,
        from_header,
        reply_to,
        subject,
        body_raw,
        headers_raw,
        raw_message,
        unsubscribe_token,
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn claim_next_mail_delivery(ctx: &ReducerContext) -> Result<(), String> {
    let mut candidates: Vec<MailDelivery> = ctx
        .db
        .mail_deliveries()
        .iter()
        .filter(|row| claimable_delivery(row, ctx.timestamp))
        .collect();

    candidates.sort_by(|left, right| {
        left.next_attempt_at
            .cmp(&right.next_attempt_at)
            .then(left.id.cmp(&right.id))
    });

    let Some(row) = candidates.into_iter().next() else {
        return Ok(());
    };

    let claimed = set_delivery_claim(row, ctx, MAIL_DELIVERY_SENDING, delivery_lease_duration());
    ctx.db.mail_deliveries().id().update(claimed);
    Ok(())
}

#[spacetimedb::reducer]
pub fn mark_mail_delivery_sent(
    ctx: &ReducerContext,
    delivery_id: String,
    smtp_status_code: Option<u16>,
    smtp_response: String,
) -> Result<(), String> {
    let Some(mut row) = ctx.db.mail_deliveries().id().find(&delivery_id) else {
        return Err(format!("Mail delivery '{delivery_id}' not found"));
    };

    if row.claim_owner != Some(ctx.sender()) {
        return Err(format!(
            "Mail delivery '{delivery_id}' is not owned by {:?}",
            ctx.sender()
        ));
    }

    row.state = MAIL_DELIVERY_SENT.to_string();
    row.sent_at = ctx.timestamp;
    row.last_error = None;
    row.claim_owner = None;
    row.claim_expires_at = Timestamp::UNIX_EPOCH;
    row.smtp_status_code = smtp_status_code;
    row.smtp_response = Some(smtp_response.clone());
    row.updated_at = ctx.timestamp;
    ctx.db.mail_deliveries().id().update(row.clone());
    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id,
        occurred_at: ctx.timestamp,
        event_type: MAIL_DELIVERY_SENT.to_string(),
        attempt_no: row.attempt_count,
        smtp_status_code,
        smtp_response: Some(smtp_response),
        error_kind: None,
        details: "Delivery accepted by SMTP server".to_string(),
        worker_identity: Some(ctx.sender()),
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn schedule_mail_delivery_retry(
    ctx: &ReducerContext,
    delivery_id: String,
    smtp_status_code: Option<u16>,
    smtp_response: String,
    error_kind: String,
) -> Result<(), String> {
    let Some(mut row) = ctx.db.mail_deliveries().id().find(&delivery_id) else {
        return Err(format!("Mail delivery '{delivery_id}' not found"));
    };

    if row.claim_owner != Some(ctx.sender()) {
        return Err(format!(
            "Mail delivery '{delivery_id}' is not owned by {:?}",
            ctx.sender()
        ));
    }

    row.last_error = Some(smtp_response.clone());
    row.smtp_status_code = smtp_status_code;
    row.smtp_response = Some(smtp_response.clone());
    row.claim_owner = None;
    row.claim_expires_at = Timestamp::UNIX_EPOCH;
    row.updated_at = ctx.timestamp;

    if row.attempt_count >= 5 {
        row.state = MAIL_DELIVERY_FAILED.to_string();
        row.sent_at = ctx.timestamp;
    } else {
        row.state = MAIL_DELIVERY_RETRY_SCHEDULED.to_string();
        row.next_attempt_at = ctx.timestamp + delivery_retry_backoff(row.attempt_count);
    }

    ctx.db.mail_deliveries().id().update(row.clone());
    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id,
        occurred_at: ctx.timestamp,
        event_type: row.state.clone(),
        attempt_no: row.attempt_count,
        smtp_status_code,
        smtp_response: Some(smtp_response.clone()),
        error_kind: Some(error_kind),
        details: format!("SMTP retry scheduled: {smtp_response}"),
        worker_identity: Some(ctx.sender()),
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn fail_mail_delivery(
    ctx: &ReducerContext,
    delivery_id: String,
    smtp_status_code: Option<u16>,
    smtp_response: String,
    error_kind: String,
) -> Result<(), String> {
    let Some(mut row) = ctx.db.mail_deliveries().id().find(&delivery_id) else {
        return Err(format!("Mail delivery '{delivery_id}' not found"));
    };

    if row.claim_owner != Some(ctx.sender()) {
        return Err(format!(
            "Mail delivery '{delivery_id}' is not owned by {:?}",
            ctx.sender()
        ));
    }

    row.state = MAIL_DELIVERY_FAILED.to_string();
    row.last_error = Some(smtp_response.clone());
    row.smtp_status_code = smtp_status_code;
    row.smtp_response = Some(smtp_response.clone());
    row.claim_owner = None;
    row.claim_expires_at = Timestamp::UNIX_EPOCH;
    row.sent_at = ctx.timestamp;
    row.updated_at = ctx.timestamp;
    ctx.db.mail_deliveries().id().update(row.clone());
    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id,
        occurred_at: ctx.timestamp,
        event_type: MAIL_DELIVERY_FAILED.to_string(),
        attempt_no: row.attempt_count,
        smtp_status_code,
        smtp_response: Some(smtp_response.clone()),
        error_kind: Some(error_kind),
        details: format!("SMTP delivery failed: {smtp_response}"),
        worker_identity: Some(ctx.sender()),
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn mark_mail_delivery_bounced(
    ctx: &ReducerContext,
    delivery_id: String,
    smtp_response: String,
    error_kind: String,
) -> Result<(), String> {
    let Some(mut row) = ctx.db.mail_deliveries().id().find(&delivery_id) else {
        return Err(format!("Mail delivery '{delivery_id}' not found"));
    };

    if row.claim_owner != Some(ctx.sender()) {
        return Err(format!(
            "Mail delivery '{delivery_id}' is not owned by {:?}",
            ctx.sender()
        ));
    }

    row.state = MAIL_DELIVERY_BOUNCED.to_string();
    row.last_error = Some(smtp_response.clone());
    row.smtp_response = Some(smtp_response.clone());
    row.claim_owner = None;
    row.claim_expires_at = Timestamp::UNIX_EPOCH;
    row.sent_at = ctx.timestamp;
    row.updated_at = ctx.timestamp;
    ctx.db.mail_deliveries().id().update(row.clone());
    ctx.db.mail_delivery_events().insert(MailDeliveryEvent {
        id: 0,
        delivery_id,
        occurred_at: ctx.timestamp,
        event_type: MAIL_DELIVERY_BOUNCED.to_string(),
        attempt_no: row.attempt_count,
        smtp_status_code: Some(550),
        smtp_response: Some(smtp_response.clone()),
        error_kind: Some(error_kind),
        details: format!("Delivery bounced: {smtp_response}"),
        worker_identity: Some(ctx.sender()),
    });
    Ok(())
}
