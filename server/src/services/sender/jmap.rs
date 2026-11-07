use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use mail_builder::{
    MessageBuilder,
    headers::{date::Date, raw::Raw},
    mime::MimePart,
};
use serde_json::{Value, json};
use spacetimedb::{
    ProcedureContext, Table, TimeDuration, Timestamp,
    http::{Request, Timeout},
};

use crate::models::{
    config::stalwart_config,
    delivery::{mail_delivery_message, system_mail_pending},
    mail_message::mail_message,
    sender::{
        JmapDeliveryState, JmapMailDelivery, JmapMailDeliveryEvent, jmap_mail_deliveries,
        jmap_mail_delivery_events,
    },
    topic::{MessageTopic, message_topics, subscription_unsubscribe_tokens, topic_app_passwords},
};
use crate::services::sender::EnvAccess as _;

const JMAP_CORE: &str = "urn:ietf:params:jmap:core";
const JMAP_MAIL: &str = "urn:ietf:params:jmap:mail";
const JMAP_SUBMISSION: &str = "urn:ietf:params:jmap:submission";
const DELIVERY_HEADER: &str = "X-SolaWi-Delivery-ID";
const OUTBOX_MAILBOX_NAME: &str = "Kommunikationszentrum JMAP delivery outbox";

const HTTP_TIMEOUT_MICROS: i64 = 30_000_000;

pub(super) struct DispatchError {
    pub message: String,
    pub permanent: bool,
}

impl DispatchError {
    fn retry(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            permanent: false,
        }
    }

    fn permanent(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            permanent: true,
        }
    }
}

struct JmapSession {
    api_url: String,
    upload_url: String,
    account_id: String,
    auth: String,
}

struct Credentials {
    jmap_url: String,
    username: String,
    password: String,
    from_email: String,
    topic: Option<MessageTopic>,
}

pub(super) fn dispatch_one(
    ctx: &mut ProcedureContext,
    delivery: &JmapMailDelivery,
    claim_token: &str,
) -> Result<(), DispatchError> {
    let credentials = resolve_credentials(ctx, delivery)?;
    let session = discover_session(ctx, &credentials)?;
    let identity_id = resolve_identity(ctx, &session, &credentials.from_email)?;
    let outbox_id = resolve_outbox(ctx, &session)?;

    let mut email_id = delivery.jmap_email_id.clone();
    if email_id.is_none() {
        if delivery.state != JmapDeliveryState::SubmissionUnknown {
            set_state(
                ctx,
                delivery,
                claim_token,
                JmapDeliveryState::Importing,
                None,
                None,
            )?;
        }
        let correlation_id = delivery.delivery_id.clone();
        email_id = find_imported_email(ctx, &session, &outbox_id, &correlation_id)?;
        if email_id.is_none() {
            let raw = compose_message(ctx, delivery, credentials.topic.as_ref())?;
            let blob_id = upload_message(ctx, &session, &raw)?;
            email_id = import_message(ctx, &session, &outbox_id, &blob_id, &correlation_id)?;
        }
        persist_imported(ctx, delivery, claim_token, email_id.as_deref())?;
    }

    let email_id =
        email_id.ok_or_else(|| DispatchError::retry("JMAP import did not produce an Email ID"))?;
    if delivery.state == JmapDeliveryState::SubmissionUnknown {
        if reconcile_submission(ctx, &session, &email_id)? {
            mark_sent(ctx, delivery, claim_token, &email_id, None)?;
            cleanup_outbox_email(ctx, &session, &email_id);
            return Ok(());
        }
    }

    set_state(
        ctx,
        delivery,
        claim_token,
        JmapDeliveryState::Submitting,
        Some(email_id.clone()),
        None,
    )?;
    let submission_id = submit_message(
        ctx,
        &session,
        &identity_id,
        &email_id,
        &credentials.from_email,
        &delivery.recipient_email,
    )?;
    mark_sent(
        ctx,
        delivery,
        claim_token,
        &email_id,
        submission_id.as_deref(),
    )?;
    cleanup_outbox_email(ctx, &session, &email_id);
    Ok(())
}

fn resolve_credentials(
    ctx: &mut ProcedureContext,
    delivery: &JmapMailDelivery,
) -> Result<Credentials, DispatchError> {
    let topic_id = delivery.topic_id;
    let system_mail_id = delivery.system_mail_id;
    let system_username = ctx.env.SYSTEM_JMAP_USERNAME();
    let system_password = ctx.env.SYSTEM_JMAP_PASSWORD();
    let message_id_domain = ctx
        .env
        .MESSAGE_ID_DOMAIN()
        .unwrap_or_else(|| "localhost".to_string());
    let values = ctx.with_tx(|tx| {
        let config = tx.db.stalwart_config().id().find(&0)?;
        if let Some(topic_id) = topic_id {
            let topic = tx.db.message_topics().id().find(&topic_id)?;
            let password_id = topic.app_password_id?;
            let password = tx.db.topic_app_passwords().id().find(&password_id)?;
            Some((
                config.jmap_url,
                topic.email_address.clone(),
                password.secret,
                topic.email_address.clone(),
                Some(topic),
            ))
        } else if system_mail_id.is_some() {
            let username = system_username.clone()?;
            let password = system_password.clone()?;
            let from_email = if username.contains('@') {
                username.clone()
            } else {
                format!("no-reply@{}", message_id_domain)
            };
            Some((config.jmap_url, username, password, from_email, None))
        } else {
            None
        }
    });
    let Some((jmap_url, username, password, from_email, topic)) = values else {
        return Err(DispatchError::permanent(
            "Missing JMAP configuration, sending account, or topic app password",
        ));
    };
    Ok(Credentials {
        jmap_url,
        username,
        password,
        from_email,
        topic,
    })
}

fn discover_session(
    ctx: &mut ProcedureContext,
    credentials: &Credentials,
) -> Result<JmapSession, DispatchError> {
    let configured = credentials.jmap_url.trim_end_matches('/');
    let root = configured.strip_suffix("/jmap").unwrap_or(configured);
    let session_url = format!("{root}/.well-known/jmap");
    let auth = format!(
        "Basic {}",
        BASE64.encode(format!("{}:{}", credentials.username, credentials.password))
    );
    let response = send_http(
        ctx,
        &session_url,
        "GET",
        &auth,
        "application/json",
        Vec::new(),
    )?;
    let api_url = response
        .get("apiUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| DispatchError::permanent("JMAP session response has no apiUrl"))?
        .to_string();
    let upload_url = response
        .get("uploadUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| DispatchError::permanent("JMAP session response has no uploadUrl"))?
        .to_string();
    let account_id = session_account_id(&response).ok_or_else(|| {
        DispatchError::permanent("JMAP session has no mail or submission account")
    })?;
    Ok(JmapSession {
        api_url,
        upload_url,
        account_id,
        auth,
    })
}

fn session_account_id(response: &Value) -> Option<String> {
    let accounts = response.get("accounts").and_then(Value::as_object);
    response
        .pointer("/primaryAccounts/urn:ietf:params:jmap:mail")
        .and_then(Value::as_str)
        .or_else(|| {
            accounts.and_then(|accounts| {
                accounts
                    .iter()
                    .find(|(_, account)| {
                        account
                            .pointer("/accountCapabilities/urn:ietf:params:jmap:mail")
                            .is_some()
                    })
                    .map(|(id, _)| id.as_str())
            })
        })
        .or_else(|| {
            response
                .pointer("/primaryAccounts/urn:ietf:params:jmap:submission")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            accounts.and_then(|accounts| {
                accounts
                    .iter()
                    .find(|(_, account)| {
                        account
                            .pointer("/accountCapabilities/urn:ietf:params:jmap:submission")
                            .is_some()
                    })
                    .map(|(id, _)| id.as_str())
            })
        })
        .map(ToString::to_string)
}

fn resolve_identity(
    ctx: &mut ProcedureContext,
    session: &JmapSession,
    from: &str,
) -> Result<String, DispatchError> {
    let response = jmap_call(
        ctx,
        session,
        JMAP_MAIL,
        "Identity/get",
        json!({
            "accountId": session.account_id,
            "properties": ["id", "email"]
        }),
        "identity",
    )?;
    let identities = result_for(&response, "Identity/get", "identity")?
        .get("list")
        .and_then(Value::as_array)
        .ok_or_else(|| DispatchError::permanent("Identity/get response has no list"))?;
    identities
        .iter()
        .find(|identity| {
            identity
                .get("email")
                .and_then(Value::as_str)
                .is_some_and(|email| email.eq_ignore_ascii_case(from))
        })
        .and_then(|identity| identity.get("id").and_then(Value::as_str))
        .map(ToString::to_string)
        .ok_or_else(|| DispatchError::permanent(format!("No authorized JMAP identity for {from}")))
}

fn outbox_query_args(account_id: &str) -> Value {
    json!({
        "accountId": account_id,
        "filter": { "name": OUTBOX_MAILBOX_NAME, "parentId": null },
        "limit": 1
    })
}

fn outbox_create_args(account_id: &str) -> Value {
    json!({
        "accountId": account_id,
        "create": { "outbox": { "name": OUTBOX_MAILBOX_NAME, "isSubscribed": false } }
    })
}

fn resolve_outbox(
    ctx: &mut ProcedureContext,
    session: &JmapSession,
) -> Result<String, DispatchError> {
    let query = jmap_call(
        ctx,
        session,
        JMAP_MAIL,
        "Mailbox/query",
        outbox_query_args(&session.account_id),
        "outbox-query",
    )?;
    let ids = result_for(&query, "Mailbox/query", "outbox-query")?
        .get("ids")
        .and_then(Value::as_array)
        .ok_or_else(|| DispatchError::permanent("Mailbox/query response has no ids"))?;
    if let Some(id) = ids.first().and_then(Value::as_str) {
        return Ok(id.to_string());
    }

    let created = jmap_call(
        ctx,
        session,
        JMAP_MAIL,
        "Mailbox/set",
        outbox_create_args(&session.account_id),
        "outbox-create",
    )?;
    result_for(&created, "Mailbox/set", "outbox-create")?
        .pointer("/created/outbox/id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| jmap_method_error(&created, "Mailbox/set", "outbox-create"))
}

fn compose_message(
    ctx: &mut ProcedureContext,
    delivery: &JmapMailDelivery,
    topic: Option<&MessageTopic>,
) -> Result<Vec<u8>, DispatchError> {
    if let Some(legacy_delivery_id) = delivery.legacy_delivery_id.as_deref() {
        let raw_message = ctx
            .with_tx(|tx| {
                tx.db
                    .mail_delivery_message()
                    .delivery_id()
                    .find(&legacy_delivery_id.to_string())
                    .map(|message| message.raw_message)
            })
            .ok_or_else(|| DispatchError::permanent("Legacy delivery message was not found"))?;
        return append_correlation_header(&raw_message, &delivery.delivery_id)
            .map(String::into_bytes)
            .map_err(DispatchError::permanent);
    }
    let source = if let Some(message_id) = delivery.mail_message_id {
        ctx.with_tx(|tx| tx.db.mail_message().id().find(&message_id))
            .ok_or_else(|| DispatchError::permanent("Canonical inbound message was not found"))?
    } else {
        return compose_system_message(
            delivery,
            ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000,
        );
    };
    let topic =
        topic.ok_or_else(|| DispatchError::permanent("Mailing-list delivery has no topic"))?;
    let token = ctx
        .with_tx(|tx| {
            delivery.subscription_id.and_then(|id| {
                tx.db
                    .subscription_unsubscribe_tokens()
                    .subscription_id()
                    .find(&id)
                    .filter(|row| row.active)
                    .map(|row| row.token)
            })
        })
        .ok_or_else(|| DispatchError::permanent("Active unsubscribe token is missing"))?;

    let delivery_id = &delivery.delivery_id;
    let hash = blake3::hash(delivery_id.as_bytes()).to_hex().to_string();
    let domain = ctx
        .env
        .MESSAGE_ID_DOMAIN()
        .unwrap_or_else(|| "localhost".to_string());
    let message_id = format!("<{hash}@{}>", safe_header(&domain));
    let unsubscribe_base = ctx.env.MAIL_UNSUBSCRIBE_BASE_URL().unwrap_or_default();
    let unsubscribe_url = if unsubscribe_base.ends_with('=') {
        format!("{unsubscribe_base}{token}")
    } else if unsubscribe_base.contains('?') {
        format!("{unsubscribe_base}&token={token}")
    } else if unsubscribe_base.ends_with("/unsubscribe") {
        format!("{unsubscribe_base}?token={token}")
    } else if !unsubscribe_base.is_empty() {
        format!(
            "{}/?unsubscribe_token={token}",
            unsubscribe_base.trim_end_matches('/')
        )
    } else {
        format!("/?unsubscribe_token={token}")
    };
    let topic_name = if topic.name.trim().is_empty() {
        topic.email_address.split('@').next().unwrap_or("list")
    } else {
        topic.name.as_str()
    };
    super::composition::compose_mail(
        &topic.email_address,
        &delivery.recipient_email,
        topic_name,
        &source.subject,
        &message_id,
        delivery_id,
        &unsubscribe_url,
        &source.headers_raw,
        &source.body_raw,
        source.date_header.as_deref(),
        ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000,
    )
    .map_err(DispatchError::permanent)
}

fn append_correlation_header(raw_message: &str, delivery_id: &str) -> Result<String, String> {
    let (headers, body) = raw_message
        .split_once("\r\n\r\n")
        .ok_or_else(|| "Legacy message has no RFC 5322 header/body separator".to_string())?;
    if headers
        .lines()
        .any(|line| line.eq_ignore_ascii_case(&format!("{DELIVERY_HEADER}: {delivery_id}")))
    {
        return Ok(raw_message.to_string());
    }
    Ok(format!(
        "{headers}\r\n{DELIVERY_HEADER}: {delivery_id}\r\n\r\n{body}"
    ))
}

fn compose_system_message(
    delivery: &JmapMailDelivery,
    date_timestamp_seconds: i64,
) -> Result<Vec<u8>, DispatchError> {
    let message_id_domain = delivery
        .from_email
        .split_once('@')
        .map(|(_, domain)| domain)
        .unwrap_or("localhost");
    let message_id = blake3::hash(delivery.delivery_id.as_bytes()).to_hex();
    let bytes = MessageBuilder::new()
        .from(delivery.from_email.as_str())
        .to(delivery.recipient_email.as_str())
        .subject(delivery.subject.clone())
        .header("Date", Date::new(date_timestamp_seconds))
        .header(
            "Message-ID",
            Raw::new(format!("<{message_id}@{}>", safe_header(message_id_domain))),
        )
        .header(DELIVERY_HEADER, Raw::new(delivery.delivery_id.clone()))
        .header("Auto-Submitted", Raw::new("auto-replied"))
        .body(MimePart::new(
            "text/plain; charset=utf-8",
            delivery.body_text.clone(),
        ))
        .write_to_string()
        .map_err(|error| {
            DispatchError::permanent(format!("Could not compose system message: {error}"))
        })?;
    Ok(bytes.into_bytes())
}

fn safe_header(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn upload_message(
    ctx: &mut ProcedureContext,
    session: &JmapSession,
    raw: &[u8],
) -> Result<String, DispatchError> {
    let url = session
        .upload_url
        .replace("{accountId}", &urlencoding::encode(&session.account_id));
    let response = send_http(
        ctx,
        &url,
        "POST",
        &session.auth,
        "message/rfc822",
        raw.to_vec(),
    )?;
    response
        .get("blobId")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| DispatchError::permanent("JMAP upload response has no blobId"))
}

fn import_message(
    ctx: &mut ProcedureContext,
    session: &JmapSession,
    outbox_id: &str,
    blob_id: &str,
    _delivery_id: &str,
) -> Result<Option<String>, DispatchError> {
    let response = jmap_call(
        ctx,
        session,
        JMAP_MAIL,
        "Email/import",
        json!({
            "accountId": session.account_id,
            "emails": { "delivery": {
                "blobId": blob_id,
                "mailboxIds": { outbox_id: true },
                "keywords": { "$seen": true }
            }}
        }),
        "email-import",
    )?;
    let result = result_for(&response, "Email/import", "email-import")?;
    if let Some(id) = result
        .pointer("/created/delivery/id")
        .and_then(Value::as_str)
    {
        return Ok(Some(id.to_string()));
    }
    Err(jmap_method_error(&response, "Email/import", "email-import"))
}

fn find_imported_email(
    ctx: &mut ProcedureContext,
    session: &JmapSession,
    outbox_id: &str,
    delivery_id: &str,
) -> Result<Option<String>, DispatchError> {
    let response = jmap_call(
        ctx,
        session,
        JMAP_MAIL,
        "Email/query",
        json!({
            "accountId": session.account_id,
            "filter": { "inMailbox": outbox_id, "header": [DELIVERY_HEADER, delivery_id] },
            "limit": 2
        }),
        "email-reconcile",
    )?;
    let ids = result_for(&response, "Email/query", "email-reconcile")?
        .get("ids")
        .and_then(Value::as_array)
        .ok_or_else(|| DispatchError::retry("Email/query response has no ids"))?;
    if ids.len() > 1 {
        return Err(DispatchError::permanent(format!(
            "More than one outbox Email matches delivery {}",
            delivery_id
        )));
    }
    Ok(ids.first().and_then(Value::as_str).map(ToString::to_string))
}

fn submit_message(
    ctx: &mut ProcedureContext,
    session: &JmapSession,
    identity_id: &str,
    email_id: &str,
    from_email: &str,
    recipient: &str,
) -> Result<Option<String>, DispatchError> {
    let response = jmap_call(
        ctx,
        session,
        JMAP_SUBMISSION,
        "EmailSubmission/set",
        json!({
            "accountId": session.account_id,
            "create": { "submission": {
                "identityId": identity_id,
                "emailId": email_id,
                "envelope": {
                    "mailFrom": { "email": from_email },
                    "rcptTo": [{ "email": recipient }]
                }
            }}
        }),
        "submission",
    )?;
    let result = result_for(&response, "EmailSubmission/set", "submission")?;
    if let Some(id) = result
        .pointer("/created/submission/id")
        .and_then(Value::as_str)
    {
        return Ok(Some(id.to_string()));
    }
    Err(jmap_method_error(
        &response,
        "EmailSubmission/set",
        "submission",
    ))
}

fn reconcile_submission(
    ctx: &mut ProcedureContext,
    session: &JmapSession,
    email_id: &str,
) -> Result<bool, DispatchError> {
    let response = jmap_call(
        ctx,
        session,
        JMAP_SUBMISSION,
        "EmailSubmission/query",
        json!({
            "accountId": session.account_id,
            "filter": { "emailIds": [email_id] },
            "limit": 1
        }),
        "submission-reconcile",
    )?;
    let ids = result_for(&response, "EmailSubmission/query", "submission-reconcile")?
        .get("ids")
        .and_then(Value::as_array)
        .ok_or_else(|| DispatchError::retry("EmailSubmission/query response has no ids"))?;
    Ok(!ids.is_empty())
}

fn persist_imported(
    ctx: &mut ProcedureContext,
    delivery: &JmapMailDelivery,
    claim_token: &str,
    email_id: Option<&str>,
) -> Result<(), DispatchError> {
    let email_id = email_id
        .ok_or_else(|| DispatchError::retry("No JMAP Email ID to persist"))?
        .to_string();
    set_state(
        ctx,
        delivery,
        claim_token,
        JmapDeliveryState::Imported,
        Some(email_id.clone()),
        None,
    )?;
    let owner = ctx.sender();
    let id = delivery.delivery_id.clone();
    let token = claim_token.to_string();
    let now = ctx.timestamp;
    ctx.with_tx(|tx| {
        tx.db
            .jmap_mail_delivery_events()
            .insert(JmapMailDeliveryEvent {
                id: 0,
                delivery_id: id.clone(),
                occurred_at: now,
                event_type: "jmap_imported".to_string(),
                attempt_no: delivery.attempt_count,
                jmap_email_id: Some(email_id.clone()),
                jmap_submission_id: None,
                details: "Email imported to the account outbox".to_string(),
                worker_identity: Some(owner),
            });
        let _ = token;
    });
    Ok(())
}

fn set_state(
    ctx: &mut ProcedureContext,
    delivery: &JmapMailDelivery,
    claim_token: &str,
    state: JmapDeliveryState,
    email_id: Option<String>,
    submission_id: Option<String>,
) -> Result<(), DispatchError> {
    let id = delivery.delivery_id.clone();
    let token = claim_token.to_string();
    let owner = ctx.sender();
    let now = ctx.timestamp;
    let updated = ctx.with_tx(|tx| {
        let Some(mut row) = tx.db.jmap_mail_deliveries().delivery_id().find(&id) else {
            return false;
        };
        if row.claim_owner != Some(owner) || row.claim_token.as_deref() != Some(token.as_str()) {
            return false;
        }
        row.state = state;
        if email_id.is_some() {
            row.jmap_email_id = email_id.clone();
        }
        if submission_id.is_some() {
            row.jmap_submission_id = submission_id.clone();
        }
        row.updated_at = now;
        tx.db.jmap_mail_deliveries().delivery_id().update(row);
        true
    });
    if updated {
        Ok(())
    } else {
        Err(DispatchError::retry(
            "Delivery claim was superseded before state update",
        ))
    }
}

fn mark_sent(
    ctx: &mut ProcedureContext,
    delivery: &JmapMailDelivery,
    claim_token: &str,
    email_id: &str,
    submission_id: Option<&str>,
) -> Result<(), DispatchError> {
    let id = delivery.delivery_id.clone();
    let token = claim_token.to_string();
    let owner = ctx.sender();
    let now = ctx.timestamp;
    let submission = submission_id.map(ToString::to_string);
    let sent = ctx.with_tx(|tx| {
        let Some(mut row) = tx.db.jmap_mail_deliveries().delivery_id().find(&id) else {
            return false;
        };
        if row.claim_owner != Some(owner) || row.claim_token.as_deref() != Some(token.as_str()) {
            return false;
        }
        row.state = JmapDeliveryState::Sent;
        row.jmap_email_id = Some(email_id.to_string());
        row.jmap_submission_id = submission.clone().or(row.jmap_submission_id);
        row.claim_owner = None;
        row.claim_token = None;
        row.claim_expires_at = Timestamp::UNIX_EPOCH;
        row.last_error = None;
        row.updated_at = now;
        tx.db
            .jmap_mail_deliveries()
            .delivery_id()
            .update(row.clone());
        tx.db
            .jmap_mail_delivery_events()
            .insert(JmapMailDeliveryEvent {
                id: 0,
                delivery_id: id.clone(),
                occurred_at: now,
                event_type: "jmap_sent".to_string(),
                attempt_no: row.attempt_count,
                jmap_email_id: Some(email_id.to_string()),
                jmap_submission_id: row.jmap_submission_id.clone(),
                details: "Stalwart accepted the JMAP submission".to_string(),
                worker_identity: Some(owner),
            });
        if let Some(system_id) = row.system_mail_id {
            tx.db.system_mail_pending().id().delete(&system_id);
        }
        true
    });
    if sent {
        Ok(())
    } else {
        Err(DispatchError::retry(
            "Delivery claim was superseded before sent state committed",
        ))
    }
}

fn cleanup_outbox_email(ctx: &mut ProcedureContext, session: &JmapSession, email_id: &str) {
    let result = jmap_call(
        ctx,
        session,
        JMAP_MAIL,
        "Email/set",
        json!({
            "accountId": session.account_id,
            "destroy": [email_id]
        }),
        "cleanup",
    );
    if let Err(error) = result {
        log::warn!(
            "Could not clean up sent JMAP Email {}: {}",
            email_id,
            error.message
        );
    }
}

fn jmap_call(
    ctx: &mut ProcedureContext,
    session: &JmapSession,
    capability: &str,
    method: &str,
    args: Value,
    call_id: &str,
) -> Result<Value, DispatchError> {
    let response = send_http(
        ctx,
        &session.api_url,
        "POST",
        &session.auth,
        "application/json",
        json!({
            "using": [JMAP_CORE, JMAP_MAIL, JMAP_SUBMISSION],
            "methodCalls": [[method, args, call_id]]
        })
        .to_string()
        .into_bytes(),
    )?;
    let _ = capability;
    Ok(response)
}

fn result_for<'a>(
    response: &'a Value,
    method: &str,
    call_id: &str,
) -> Result<&'a Value, DispatchError> {
    let calls = response
        .get("methodResponses")
        .and_then(Value::as_array)
        .ok_or_else(|| DispatchError::retry("JMAP response is missing methodResponses"))?;
    for call in calls {
        if call.get(2).and_then(Value::as_str) != Some(call_id) {
            continue;
        }
        let response_method = call.get(0).and_then(Value::as_str);
        let result = call
            .get(1)
            .ok_or_else(|| DispatchError::retry("JMAP method result is missing"))?;
        if response_method == Some(method) {
            return Ok(result);
        }
        if response_method == Some("error") {
            return Err(parse_method_error(method, result));
        }
    }
    Err(DispatchError::retry(format!(
        "JMAP response has no {method} result for call {call_id}"
    )))
}

fn jmap_method_error(response: &Value, method: &str, call_id: &str) -> DispatchError {
    match result_for(response, method, call_id) {
        Ok(result) => parse_method_error(method, result),
        Err(error) => error,
    }
}

fn parse_method_error(method: &str, result: &Value) -> DispatchError {
    let error_type = result
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let detail = result
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or(error_type);
    let permanent = !matches!(
        error_type,
        "serverFail" | "serverPartialFail" | "rateLimit" | "temporarilyUnavailable"
    );
    DispatchError {
        message: format!("{method} failed ({error_type}): {detail}"),
        permanent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_account_id_falls_back_to_primary_submission_account() {
        let session = json!({
            "accounts": {
                "l": {
                    "accountCapabilities": {
                        "urn:ietf:params:jmap:submission": {}
                    }
                }
            },
            "primaryAccounts": {
                "urn:ietf:params:jmap:submission": "l"
            }
        });

        assert_eq!(session_account_id(&session).as_deref(), Some("l"));
    }

    #[test]
    fn session_account_id_prefers_primary_mail_account() {
        let session = json!({
            "accounts": {
                "mail": { "accountCapabilities": { "urn:ietf:params:jmap:mail": {} } },
                "submission": { "accountCapabilities": { "urn:ietf:params:jmap:submission": {} } }
            },
            "primaryAccounts": {
                "urn:ietf:params:jmap:mail": "mail",
                "urn:ietf:params:jmap:submission": "submission"
            }
        });

        assert_eq!(session_account_id(&session).as_deref(), Some("mail"));
    }

    #[test]
    fn outbox_mailbox_uses_a_roleless_jmap_mailbox() {
        let query = outbox_query_args("account");
        let create = outbox_create_args("account");

        assert_eq!(
            query.pointer("/filter/name").and_then(Value::as_str),
            Some(OUTBOX_MAILBOX_NAME)
        );
        assert_eq!(query.pointer("/filter/parentId"), Some(&Value::Null));
        assert_eq!(
            create
                .pointer("/create/outbox/name")
                .and_then(Value::as_str),
            Some(OUTBOX_MAILBOX_NAME)
        );
        assert!(create.pointer("/create/outbox/role").is_none());
    }

    #[test]
    fn result_for_reports_jmap_method_errors() {
        let response = json!({
            "methodResponses": [[
                "error",
                { "type": "invalidArguments", "description": "Unsupported filter" },
                "outbox-query"
            ]]
        });

        let error = result_for(&response, "Mailbox/query", "outbox-query").unwrap_err();

        assert_eq!(
            error.message,
            "Mailbox/query failed (invalidArguments): Unsupported filter"
        );
        assert!(error.permanent);
    }
}

fn send_http(
    ctx: &mut ProcedureContext,
    url: &str,
    method: &str,
    auth: &str,
    content_type: &str,
    body: Vec<u8>,
) -> Result<Value, DispatchError> {
    let request = Request::builder()
        .uri(url)
        .method(method)
        .header("Authorization", auth)
        .header("Content-Type", content_type)
        .extension(Timeout(TimeDuration::from_micros(HTTP_TIMEOUT_MICROS)))
        .body(body)
        .map_err(|error| {
            DispatchError::permanent(format!("Could not build Stalwart request: {error}"))
        })?;
    let response = ctx
        .http
        .send(request)
        .map_err(|error| DispatchError::retry(format!("Stalwart HTTP request failed: {error}")))?;
    let (parts, body) = response.into_parts();
    let status = parts.status.as_u16();
    let bytes = body.into_bytes();
    if !(200..300).contains(&status) {
        let detail = String::from_utf8_lossy(&bytes)
            .chars()
            .take(300)
            .collect::<String>();
        return Err(DispatchError {
            message: format!("Stalwart returned HTTP {status}: {detail}"),
            permanent: (400..500).contains(&status) && status != 408 && status != 429,
        });
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| DispatchError::retry(format!("Stalwart returned invalid JSON: {error}")))
}
