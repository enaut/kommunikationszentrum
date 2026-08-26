use chrono::Utc;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::Error as SmtpError;
use lettre::SmtpTransport;
use opentelemetry::trace;
use regex::Regex;
use serde_json::to_string;
use std::error::Error;
use tracing::trace;

use crate::config::SenderConfig;
use crate::module_bindings::{
    MailMessage, MessageCategory, Subscription, SubscriptionUnsubscribeToken,
};

pub fn build_transport(config: &SenderConfig) -> Result<SmtpTransport, Box<dyn Error>> {
    let mut builder = if config.smtp_use_tls {
        SmtpTransport::relay(&config.smtp_host)?
    } else {
        SmtpTransport::builder_dangerous(&config.smtp_host)
    };

    builder = builder.port(config.smtp_port);

    if let (Some(username), Some(password)) = (&config.smtp_username, &config.smtp_password) {
        builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
    }

    Ok(builder.build())
}

pub fn is_transient_error(error: &SmtpError) -> bool {
    error.is_transient() || error.is_timeout()
}

pub fn is_permanent_error(error: &SmtpError) -> bool {
    error.is_permanent()
}

pub fn compose_delivery(
    config: &SenderConfig,
    ingress_id: &str,
    message: &MailMessage,
    subscription: &Subscription,
    category: &MessageCategory,
    token: &SubscriptionUnsubscribeToken,
) -> Result<(String, String), Box<dyn Error>> {
    trace!("Composing delivery for {ingress_id}");

    let list_email = category.email_address.clone();
    let list_name = if category.name.trim().is_empty() {
        category
            .email_address
            .split('@')
            .next()
            .unwrap_or("list")
            .to_string()
    } else {
        category.name.clone()
    };
    trace!("List email: {list_email}, list name: {list_name}");
    let recipient_email = subscription.subscriber_email.clone();
    let subject = rewrite_subject(&list_name, &message.subject);
    let reply_to = message.sender_email.clone();
    let message_id = format!(
        "<{}@{}>",
        message_id_seed(ingress_id, &recipient_email),
        config.message_id_domain
    );
    let date = Utc::now().to_rfc2822();
    let unsubscribe_url = format!("{}?token={}", config.unsubscribe_base_url, token.token);
    trace!("Unsubscribe url {unsubscribe_url}");

    trace!("Writing list-mail for {list_email} to {recipient_email}");

    let headers = vec![
        ("From".to_string(), list_email.clone()),
        ("To".to_string(), recipient_email.clone()),
        ("Reply-To".to_string(), reply_to),
        ("Subject".to_string(), subject),
        ("Message-ID".to_string(), message_id),
        ("Date".to_string(), date),
        (
            "List-Id".to_string(),
            format!("{} <{}>", list_name, list_email),
        ),
        ("List-Post".to_string(), format!("<mailto:{}>", list_email)),
        (
            "List-Unsubscribe".to_string(),
            format!(
                "<mailto:{}?subject=unsubscribe>, <{}>",
                list_email, unsubscribe_url
            ),
        ),
        (
            "List-Unsubscribe-Post".to_string(),
            "List-Unsubscribe=One-Click".to_string(),
        ),
        ("Precedence".to_string(), "list".to_string()),
        ("Sender".to_string(), list_email.clone()),
        ("X-Mailing-List".to_string(), list_name.clone()),
        ("X-BeenThere".to_string(), list_email),
    ];

    let headers_raw = to_string(&headers)?;
    let raw_message = render_raw_message(&headers, &message.body_raw);
    Ok((headers_raw, raw_message))
}

fn sanitize_header_value(value: &str) -> String {
    value.replace(['\r', '\n'], "")
}

/// Regex matching a single leading reply/forward tag, with optional
/// bracketed/parenthesized counter like "RE[2]:" or "FW(3):", and optional
/// space before the colon.
fn tag_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?ix)
            ^\s*
            (?P<tag>re|aw|sv|rif|res|tr|rv|wg|fwd|fw)
            (?:\s*[\[\(]\s*\d+\s*[\]\)])?   # optional [2] or (2)
            \s*:\s*
            ",
        )
        .unwrap()
    })
}

fn is_forward_tag(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "fwd" | "fw" | "wg" | "tr" | "rv"
    )
}

/// Strips all leading Re/Fwd-style prefixes (any supported locale, any order,
/// any count) and reports whether a reply and/or forward tag was seen.
fn strip_reply_fwd_prefixes(subject: &str) -> (bool, bool, &str) {
    trace!("Strip reply/fwd prefixes from {subject}");

    let mut rest = subject;
    let mut saw_reply = false;
    let mut saw_fwd = false;

    while let Some(caps) = tag_re().captures(rest) {
        let tag = &caps["tag"];
        if is_forward_tag(tag) {
            saw_fwd = true;
        } else {
            saw_reply = true;
        }
        let m = caps.get(0).unwrap();
        rest = &rest[m.end()..];
    }
    trace!("Saw reply: {saw_reply}, saw fwd: {saw_fwd}, rest: {rest}");
    (saw_reply, saw_fwd, rest)
}

fn rewrite_subject(list_name: &str, subject: &str) -> String {
    let prefix = format!("[{list_name}]: ");
    let lower_prefix = prefix.to_ascii_lowercase();

    let (saw_reply, saw_fwd, core) = strip_reply_fwd_prefixes(subject);

    // Canonical, deduped reply/fwd marker (Re: before Fwd:, matching the
    // usual convention of "reply to a forward").
    let mut canonical_tags = String::new();
    if saw_reply {
        canonical_tags.push_str("Re: ");
    }
    if saw_fwd {
        canonical_tags.push_str("Fwd: ");
    }

    let new_subject;

    let lower_core = core.to_ascii_lowercase();
    if lower_core.starts_with(&lower_prefix) {
        // Replace whatever casing/spacing variant was there with the
        // canonical prefix, keeping everything after it untouched.
        let rest_after_tag = &core[prefix.len()..];
        new_subject = format!("{canonical_tags}{prefix}{rest_after_tag}");
    } else {
        new_subject = format!("{canonical_tags}{prefix}{core}")
    }
    trace!("New subject: {new_subject}");
    new_subject
}

fn render_raw_message(headers: &[(String, String)], body: &str) -> String {
    let mut raw = String::new();
    for (name, value) in headers {
        raw.push_str(&sanitize_header_value(name));
        raw.push_str(": ");
        raw.push_str(&sanitize_header_value(value));
        raw.push_str("\r\n");
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    raw
}

fn message_id_seed(ingress_id: &str, recipient_email: &str) -> String {
    format!(
        "{}-{}",
        ingress_id.replace(':', "-"),
        recipient_email.replace('@', "-at-")
    )
}
