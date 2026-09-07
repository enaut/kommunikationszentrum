use lettre::message::header::{Header, HeaderName, HeaderValue};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::transport::smtp::Error as SmtpError;
use lettre::{AsyncSmtpTransport, Message, Tokio1Executor};
use regex::Regex;
use spacetimedb_sdk::Table as _;
use std::error::Error;
use tracing::{trace, warn};

use crate::config::SenderConfig;
use crate::module_bindings::{
    DbConnection, MailMessage, MessageCategory, Subscription, SubscriptionUnsubscribeToken,
    VisibleCategoryAppPasswordsTableAccess as _, VisibleMessageCategoriesTableAccess as _,
};

// ---------------------------------------------------------------------------
// Custom mailing-list header types for the lettre `Message` builder
// ---------------------------------------------------------------------------

macro_rules! custom_header {
    ($type_name:ident, $header_str:literal) => {
        #[derive(Debug, Clone)]
        struct $type_name(String);

        impl Header for $type_name {
            fn name() -> HeaderName {
                HeaderName::new_from_ascii_str($header_str)
            }

            fn parse(s: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                Ok(Self(s.to_string()))
            }

            fn display(&self) -> HeaderValue {
                HeaderValue::new(
                    HeaderName::new_from_ascii_str($header_str),
                    self.0.clone(),
                )
            }
        }
    };
}

custom_header!(ListId, "List-Id");
custom_header!(ListPost, "List-Post");
custom_header!(ListUnsubscribe, "List-Unsubscribe");
custom_header!(ListUnsubscribePost, "List-Unsubscribe-Post");
custom_header!(PrecedenceHeader, "Precedence");
custom_header!(SenderHeader, "Sender");
custom_header!(XMailingList, "X-Mailing-List");
custom_header!(XBeenThere, "X-BeenThere");

// ---------------------------------------------------------------------------
// SMTP transport (async with connection pooling)
// ---------------------------------------------------------------------------

pub fn build_transport(
    config: &SenderConfig,
    username: &str,
    password: &str,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, Box<dyn Error>> {
    let mut builder = if config.smtp_use_tls {
        let tls = if config.smtp_accept_invalid_certs || config.smtp_accept_invalid_hostnames {
            let mut tls_builder = TlsParameters::builder(config.smtp_host.clone());

            if config.smtp_accept_invalid_certs {
                tls_builder = tls_builder.dangerous_accept_invalid_certs(true);
            }

            if config.smtp_accept_invalid_hostnames {
                tls_builder = tls_builder.dangerous_accept_invalid_hostnames(true);
            }

            Tls::Required(tls_builder.build()?)
        } else {
            let tls_parameters = TlsParameters::builder(config.smtp_host.clone()).build()?;
            Tls::Required(tls_parameters)
        };

        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)?.tls(tls)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp_host)
    };

    builder = builder.port(config.smtp_port);
    builder = builder.credentials(Credentials::new(username.to_owned(), password.to_owned()));

    Ok(builder.build())
}

pub fn resolve_category_smtp_credentials(
    connection: &DbConnection,
    category_id: u64,
) -> Result<(String, String), Box<dyn Error>> {
    let category = connection
        .db
        .visible_message_categories()
        .iter()
        .find(|category| category.id == category_id)
        .ok_or_else(|| format!("Category {category_id} not in local cache"))?;

    let app_password_id = category
        .app_password_id
        .ok_or_else(|| format!("Category {category_id} has no SMTP app password"))?;

    let app_password = connection
        .db
        .visible_category_app_passwords()
        .id()
        .find(&app_password_id)
        .ok_or_else(|| {
            format!("App password {app_password_id} for category {category_id} not in local cache")
        })?;
    Ok((category.email_address, app_password.secret))
}

pub fn is_permanent_error(error: &SmtpError) -> bool {
    error.is_permanent()
}

// ---------------------------------------------------------------------------
// Message composition (lettre builder pattern)
// ---------------------------------------------------------------------------

/// Compose a per-recipient mailing-list delivery using the lettre `Message`
/// builder.  Returns the fully formatted RFC 5322 message as a `String`,
/// ready for storage in SpacetimeDB and later SMTP submission.
pub fn compose_delivery(
    config: &SenderConfig,
    ingress_id: &str,
    message: &MailMessage,
    subscription: &Subscription,
    category: &MessageCategory,
    token: &SubscriptionUnsubscribeToken,
) -> Result<String, Box<dyn Error>> {
    trace!("Composing delivery for {ingress_id}");

    let list_email = &category.email_address;
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

    let recipient_email = &subscription.subscriber_email;
    let subject = rewrite_subject(&list_name, &message.subject);
    let reply_to = &message.sender_email;
    let msg_id = format!(
        "<{}@{}>",
        message_id_seed(ingress_id, recipient_email),
        config.message_id_domain
    );
    let unsubscribe_url = format!("{}?token={}", config.unsubscribe_base_url, token.token);
    trace!("Unsubscribe url {unsubscribe_url}");

    trace!("Building list-mail for {list_email} to {recipient_email}");

    let to_addr_res: Result<lettre::Address, _> = recipient_email.parse();
    let from_addr_res: Result<lettre::Address, _> = list_email.parse();
    let reply_to_res: Result<lettre::Address, _> = reply_to.parse();

    let raw_message = match (from_addr_res, to_addr_res, reply_to_res) {
        (Ok(from_addr), Ok(to_addr), Ok(reply_to_addr)) => {
            let email = Message::builder()
                .from(from_addr.into())
                .to(to_addr.into())
                .reply_to(reply_to_addr.into())
                .subject(subject)
                .message_id(Some(msg_id))
                .header(ListId(format!("{list_name} <{list_email}>")))
                .header(ListPost(format!("<mailto:{list_email}>")))
                .header(ListUnsubscribe(format!(
                    "<mailto:{list_email}?subject=unsubscribe>, <{unsubscribe_url}>"
                )))
                .header(ListUnsubscribePost(
                    "List-Unsubscribe=One-Click".to_string(),
                ))
                .header(PrecedenceHeader("list".to_string()))
                .header(SenderHeader(list_email.clone()))
                .header(XMailingList(list_name))
                .header(XBeenThere(list_email.clone()))
                .body(message.body_raw.clone())?;

            String::from_utf8(email.formatted().to_vec())?
        }
        _ => {
            warn!(
                "Recipient or sender email is not a valid RFC 5322 address (recipient: '{recipient_email}', list: '{list_email}', reply-to: '{reply_to}'). Falling back to raw headers."
            );
            render_fallback_raw_message(
                list_email,
                recipient_email,
                reply_to,
                &subject,
                &msg_id,
                &list_name,
                &unsubscribe_url,
                &message.body_raw,
            )
        }
    };

    trace!("Composed message ({} bytes)", raw_message.len());
    Ok(raw_message)
}

fn sanitize_header_value(value: &str) -> String {
    value.replace(['\r', '\n'], "")
}

fn render_fallback_raw_message(
    from: &str,
    to: &str,
    reply_to: &str,
    subject: &str,
    msg_id: &str,
    list_name: &str,
    unsubscribe_url: &str,
    body: &str,
) -> String {
    let from_clean = sanitize_header_value(from);
    let to_clean = sanitize_header_value(to);
    let reply_to_clean = sanitize_header_value(reply_to);
    let subject_clean = sanitize_header_value(subject);
    let msg_id_clean = sanitize_header_value(msg_id);
    let list_name_clean = sanitize_header_value(list_name);
    let unsubscribe_url_clean = sanitize_header_value(unsubscribe_url);

    format!(
        "From: {from_clean}\r\n\
         To: {to_clean}\r\n\
         Reply-To: {reply_to_clean}\r\n\
         Subject: {subject_clean}\r\n\
         Message-ID: {msg_id_clean}\r\n\
         List-Id: {list_name_clean} <{from_clean}>\r\n\
         List-Post: <mailto:{from_clean}>\r\n\
         List-Unsubscribe: <mailto:{from_clean}?subject=unsubscribe>, <{unsubscribe_url_clean}>\r\n\
         List-Unsubscribe-Post: List-Unsubscribe=One-Click\r\n\
         Precedence: list\r\n\
         Sender: {from_clean}\r\n\
         X-Mailing-List: {list_name_clean}\r\n\
         X-BeenThere: {from_clean}\r\n\
         \r\n\
         {body}"
    )
}

// ---------------------------------------------------------------------------
// Subject rewriting
// ---------------------------------------------------------------------------

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

fn message_id_seed(ingress_id: &str, recipient_email: &str) -> String {
    format!(
        "{}-{}",
        ingress_id.replace(':', "-"),
        recipient_email.replace('@', "-at-")
    )
}
