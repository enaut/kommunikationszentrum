use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use lettre::message::header::{Header, HeaderName, HeaderValue};
use lettre::transport::smtp::Error as SmtpError;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, Message, Tokio1Executor};
use mail_parser::{MessageParser, PartType};
use regex::Regex;
use std::collections::HashSet;
use std::error::Error;
use tracing::{trace, warn};

use crate::config::SenderConfig;
use crate::module_bindings::{
    DbConnection, MailMessage, MessageTopic, SenderMessageTopicsTableAccess as _,
    SenderTopicAppPasswordsTableAccess as _, Subscription, SubscriptionUnsubscribeToken,
};

// ---------------------------------------------------------------------------
// Custom mailing-list header types for the lettre `Message` builder
// ---------------------------------------------------------------------------

macro_rules! custom_header {
    ($type_name:ident, $header_str:literal) => {
        #[derive(Debug, Clone)]
        pub struct $type_name(pub String);

        impl Header for $type_name {
            fn name() -> HeaderName {
                HeaderName::new_from_ascii_str($header_str)
            }

            fn parse(s: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                Ok(Self(s.to_string()))
            }

            fn display(&self) -> HeaderValue {
                HeaderValue::new(HeaderName::new_from_ascii_str($header_str), self.0.clone())
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
custom_header!(AutoSubmitted, "Auto-Submitted");

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

pub fn resolve_topic_smtp_credentials(
    connection: &DbConnection,
    topic_id: u64,
) -> Result<(String, String), Box<dyn Error>> {
    let topic = connection
        .db
        .sender_message_topics()
        .id()
        .find(&topic_id)
        .ok_or_else(|| format!("Topic {topic_id} not in local cache"))?;

    let app_password_id = topic
        .app_password_id
        .ok_or_else(|| format!("Topic {topic_id} has no SMTP app password"))?;

    let app_password = connection
        .db
        .sender_topic_app_passwords()
        .id()
        .find(&app_password_id)
        .ok_or_else(|| {
            format!("App password {app_password_id} for topic {topic_id} not in local cache")
        })?;
    Ok((topic.email_address, app_password.secret))
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
    _subscription: &Subscription,
    topic: &MessageTopic,
    token: &SubscriptionUnsubscribeToken,
    subscriber_email: &str,
) -> Result<String, Box<dyn Error>> {
    trace!("Composing delivery for {ingress_id}");

    let list_email = &topic.email_address;
    let list_name = if topic.name.trim().is_empty() {
        topic
            .email_address
            .split('@')
            .next()
            .unwrap_or("list")
            .to_string()
    } else {
        topic.name.clone()
    };
    trace!("List email: {list_email}, list name: {list_name}");

    let recipient_email = subscriber_email;
    let subject = clean_unsubscribe_content(&rewrite_subject(&list_name, &message.subject));
    let reply_to = &message.sender_email;
    let msg_id = format!(
        "<{}@{}>",
        message_id_seed(ingress_id, recipient_email),
        config.message_id_domain
    );
    let unsubscribe_url = build_unsubscribe_url(&config.unsubscribe_base_url, &token.token);
    trace!("Unsubscribe url {unsubscribe_url}");

    trace!("Building list-mail for {list_email} to {recipient_email}");

    let to_addr_res: Result<lettre::Address, _> = recipient_email.parse();
    let from_addr_res: Result<lettre::Address, _> = list_email.parse();
    let reply_to_res: Result<lettre::Address, _> = reply_to.parse();

    let (mime_headers, mime_body) = prepare_mime_entity(message)?;

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
                .body(String::new())?;

            let formatted = String::from_utf8(email.formatted().to_vec())?;
            let header_block = formatted
                .split_once("\r\n\r\n")
                .map(|(headers, _)| headers)
                .ok_or("Lettre produced a message without a header/body separator")?;
            assemble_raw_message(header_block, &mime_headers, &mime_body)?
        }
        _ => {
            warn!(
                "Recipient or sender email is not a valid RFC 5322 address (recipient: '{recipient_email}', list: '{list_email}', reply-to: '{reply_to}'). Falling back to raw headers."
            );
            let fallback = render_fallback_raw_message(
                list_email,
                recipient_email,
                reply_to,
                &subject,
                &msg_id,
                &list_name,
                &unsubscribe_url,
                "",
            );
            let header_block = fallback
                .split_once("\r\n\r\n")
                .map(|(headers, _)| headers)
                .ok_or("Fallback message is missing a header/body separator")?;
            assemble_raw_message(header_block, &mime_headers, &mime_body)?
        }
    };

    trace!("Composed message ({} bytes)", raw_message.len());
    Ok(raw_message)
}

fn sanitize_header_value(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn token_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)sub-[0-9]+-[0-9a-f]{32}").unwrap())
}

fn unsubscribe_url_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)(?:https?://|mailto:|/)[^\s<>\"']*sub-[0-9]+-[0-9a-f]{32}[^\s<>\"']*"#)
            .unwrap()
    })
}

fn unsubscribe_anchor_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?is)<a\b[^>]*\bhref\s*=\s*[\"'][^\"']*sub-[0-9]+-[0-9a-f]{32}[^\"']*[\"'][^>]*>.*?</a\s*>"#,
        )
        .unwrap()
    })
}

fn clean_unsubscribe_content(value: &str) -> String {
    let without_links = unsubscribe_anchor_re().replace_all(value, "");
    let without_urls = unsubscribe_url_re().replace_all(&without_links, "");
    token_re().replace_all(&without_urls, "").into_owned()
}

fn prepare_mime_entity(message: &MailMessage) -> Result<(String, Vec<u8>), Box<dyn Error>> {
    let original_headers: Vec<(String, String)> = if message.headers_raw.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&message.headers_raw)?
    };
    let mut entity = String::new();
    let mut has_content_type = false;
    let mut has_mime_version = false;

    for (name, value) in original_headers {
        let lower_name = name.to_ascii_lowercase();
        if lower_name == "mime-version" || lower_name.starts_with("content-") {
            if lower_name == "content-type" {
                has_content_type = true;
            } else if lower_name == "mime-version" {
                has_mime_version = true;
            }
            entity.push_str(&name);
            entity.push_str(": ");
            let value = if lower_name == "content-type" {
                value
            } else {
                clean_unsubscribe_content(&value)
            };
            entity.push_str(&sanitize_header_value(&value));
            entity.push_str("\r\n");
        }
    }
    if !has_mime_version {
        entity.push_str("MIME-Version: 1.0\r\n");
    }
    if !has_content_type {
        entity.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    }
    entity.push_str("\r\n");
    entity.push_str(&message.body_raw);

    let Some(parsed) = MessageParser::default().parse(entity.as_bytes()) else {
        return Err("Could not parse original MIME entity".into());
    };

    let mut part_ids = HashSet::new();
    let mut cleaned_text_parts = std::collections::HashMap::new();
    for id in parsed.text_body.iter().chain(parsed.html_body.iter()) {
        if !part_ids.insert(*id) {
            continue;
        }
        let Some(part) = parsed.parts.get(*id as usize) else {
            continue;
        };
        let (text, is_html) = match &part.body {
            PartType::Text(text) => (text.as_ref(), false),
            PartType::Html(html) => (html.as_ref(), true),
            _ => continue,
        };
        let cleaned = clean_unsubscribe_content(text);
        if cleaned != text {
            if part.is_encoding_problem {
                return Err("Cannot safely clean an incorrectly encoded MIME text part".into());
            }
            cleaned_text_parts.insert(*id, (cleaned, is_html));
        }
    }

    let mut patches = Vec::new();
    for (id, part) in parsed.parts.iter().enumerate() {
        let start = part.offset_header as usize;
        let body_start = part.offset_body as usize;
        let end = part.offset_end as usize;
        if start > body_start || body_start > end || end > entity.len() {
            return Err("MIME parser returned invalid part offsets".into());
        }

        let cleaned_body = cleaned_text_parts.get(&(id as u32));
        let raw_headers = &entity.as_bytes()[start..body_start];
        let fields = parse_header_fields(std::str::from_utf8(raw_headers)?);
        let header_needs_cleaning = fields.iter().any(|(name, value)| {
            !(name.eq_ignore_ascii_case("content-type")
                && value.to_ascii_lowercase().starts_with("multipart/"))
                && clean_unsubscribe_content(value) != *value
        });

        if cleaned_body.is_none() && !header_needs_cleaning {
            continue;
        }

        let is_html = cleaned_body.is_some_and(|(_, is_html)| *is_html);
        let rewrite_body = cleaned_body.is_some();
        let new_headers = rewrite_mime_part_headers(raw_headers, rewrite_body, is_html)?;
        let mut header_replacement = new_headers.into_bytes();
        header_replacement.extend_from_slice(b"\r\n");
        patches.push((start, body_start, header_replacement));

        if let Some((cleaned, _)) = cleaned_body {
            patches.push((body_start, end, base64_mime_body(cleaned.as_bytes())));
        }
    }

    let mut entity_bytes = entity.into_bytes();
    patches.sort_by(|a, b| b.0.cmp(&a.0));
    for (start, end, replacement) in patches {
        entity_bytes.splice(start..end, replacement);
    }

    let entity = String::from_utf8(entity_bytes)?;
    let (mime_headers, mime_body) = entity
        .split_once("\r\n\r\n")
        .ok_or("Prepared MIME entity is missing a header/body separator")?;
    Ok((mime_headers.to_string(), mime_body.as_bytes().to_vec()))
}

fn rewrite_mime_part_headers(
    raw_headers: &[u8],
    rewrite_body: bool,
    is_html: bool,
) -> Result<String, Box<dyn Error>> {
    let raw_headers = std::str::from_utf8(raw_headers)?;
    let fields = parse_header_fields(raw_headers);
    let mut result = String::new();
    let mut saw_content_type = false;
    let mut saw_transfer_encoding = false;

    for (name, value) in fields {
        if name.eq_ignore_ascii_case("content-transfer-encoding") {
            if !saw_transfer_encoding {
                let value = if rewrite_body {
                    "base64".to_string()
                } else {
                    clean_unsubscribe_content(&value)
                };
                result.push_str("Content-Transfer-Encoding: ");
                result.push_str(&sanitize_header_value(&value));
                result.push_str("\r\n");
                saw_transfer_encoding = true;
            }
        } else if name.eq_ignore_ascii_case("content-type") {
            if !saw_content_type {
                let value = if rewrite_body {
                    content_type_with_utf8_charset(&clean_unsubscribe_content(&value), is_html)
                } else if value.to_ascii_lowercase().starts_with("multipart/") {
                    value
                } else {
                    clean_unsubscribe_content(&value)
                };
                result.push_str("Content-Type: ");
                result.push_str(&sanitize_header_value(&value));
                result.push_str("\r\n");
                saw_content_type = true;
            }
        } else {
            result.push_str(&name);
            result.push_str(": ");
            result.push_str(&sanitize_header_value(&clean_unsubscribe_content(&value)));
            result.push_str("\r\n");
        }
    }

    if rewrite_body && !saw_content_type {
        result.push_str(if is_html {
            "Content-Type: text/html; charset=utf-8\r\n"
        } else {
            "Content-Type: text/plain; charset=utf-8\r\n"
        });
    }
    if rewrite_body && !saw_transfer_encoding {
        result.push_str("Content-Transfer-Encoding: base64\r\n");
    }
    Ok(result)
}

fn parse_header_fields(raw_headers: &str) -> Vec<(String, String)> {
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in raw_headers.trim_end_matches(['\r', '\n']).split("\r\n") {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some((_, value)) = fields.last_mut() {
                value.push(' ');
                value.push_str(line.trim());
            }
        } else if let Some((name, value)) = line.split_once(':') {
            fields.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    fields
}

fn content_type_with_utf8_charset(value: &str, is_html: bool) -> String {
    let mut segments = value.split(';');
    let media_type = segments.next().unwrap_or_default().trim();
    let media_type = if media_type.contains('/') {
        media_type.to_string()
    } else if is_html {
        "text/html".to_string()
    } else {
        "text/plain".to_string()
    };
    let parameters = segments
        .map(str::trim)
        .filter(|parameter| {
            parameter
                .split_once('=')
                .is_none_or(|(name, _)| !name.trim().eq_ignore_ascii_case("charset"))
        })
        .collect::<Vec<_>>();
    if parameters.is_empty() {
        format!("{media_type}; charset=utf-8")
    } else {
        format!("{media_type}; {}; charset=utf-8", parameters.join("; "))
    }
}

fn base64_mime_body(bytes: &[u8]) -> Vec<u8> {
    let encoded = BASE64.encode(bytes);
    let mut wrapped = Vec::with_capacity(encoded.len() + encoded.len() / 76 * 2);
    for (index, line) in encoded.as_bytes().chunks(76).enumerate() {
        if index > 0 {
            wrapped.extend_from_slice(b"\r\n");
        }
        wrapped.extend_from_slice(line);
    }
    wrapped
}

fn assemble_raw_message(
    generated_headers: &str,
    mime_headers: &str,
    mime_body: &[u8],
) -> Result<String, Box<dyn Error>> {
    let mut raw = strip_generated_mime_headers(generated_headers);
    for line in mime_headers.split("\r\n").filter(|line| !line.is_empty()) {
        raw.push_str(line);
        raw.push_str("\r\n");
    }
    raw.push_str("\r\n");
    raw.push_str(std::str::from_utf8(mime_body)?);
    Ok(raw)
}

fn strip_generated_mime_headers(headers: &str) -> String {
    let mut result = String::new();
    let mut keep_current = true;
    for line in headers.split("\r\n").filter(|line| !line.is_empty()) {
        if line.starts_with(' ') || line.starts_with('\t') {
            if keep_current {
                result.push_str(line);
                result.push_str("\r\n");
            }
            continue;
        }
        let name = line
            .split_once(':')
            .map(|(name, _)| name.trim())
            .unwrap_or("");
        keep_current = !matches!(
            name.to_ascii_lowercase().as_str(),
            "content-type" | "content-transfer-encoding" | "mime-version"
        );
        if keep_current {
            result.push_str(line);
            result.push_str("\r\n");
        }
    }
    result
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

pub fn build_unsubscribe_url(base_url: &str, token: &str) -> String {
    if base_url.ends_with('=') {
        format!("{base_url}{token}")
    } else if base_url.contains('?') {
        format!("{base_url}&token={token}")
    } else {
        let base = base_url.trim_end_matches('/');
        if base.ends_with("/unsubscribe") {
            format!("{base}?token={token}")
        } else {
            format!("{base}/?unsubscribe_token={token}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_mail_message(headers: &str, body: &str) -> MailMessage {
        MailMessage {
            id: 1,
            queue_id: None,
            received_at: spacetimedb_sdk::Timestamp::UNIX_EPOCH,
            sender_account_id: None,
            sender_email: "sender@example.test".to_string(),
            subject: "Test message".to_string(),
            from_header: "Sender <sender@example.test>".to_string(),
            reply_to: None,
            date_header: None,
            message_id: None,
            cc_header: None,
            headers_raw: headers.to_string(),
            body_raw: body.to_string(),
            message_size: body.len() as u64,
        }
    }

    #[test]
    fn prepare_mime_entity_preserves_multipart_attachments_and_redacts_tokens() {
        let token = "sub-7-0123456789abcdef0123456789abcdef";
        let html = format!(
            "<html><body><p>HTML reply</p><a href=\"https://list.example/unsubscribe?token={token}\">unsubscribe</a></body></html>"
        );
        let html_base64 = BASE64.encode(html.as_bytes());
        let headers = serde_json::json!([
            ["MIME-Version", "1.0"],
            [
                "Content-Type",
                "multipart/mixed; boundary=\"mixed-boundary\""
            ]
        ])
        .to_string();
        let body = format!(
            "This is a multipart message.\r\n\
             --mixed-boundary\r\n\
             Content-Type: multipart/alternative; boundary=\"alternative-boundary\"\r\n\r\n\
             --alternative-boundary\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\
             Content-Transfer-Encoding: quoted-printable\r\n\r\n\
             Quoted reply with token {token} and https://list.example/unsubscribe?token={token}\r\n\
             --alternative-boundary\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Content-Transfer-Encoding: base64\r\n\r\n\
             {html_base64}\r\n\
             --alternative-boundary--\r\n\
             --mixed-boundary\r\n\
             Content-Type: application/octet-stream\r\n\
             Content-Transfer-Encoding: base64\r\n\
             Content-Disposition: attachment; filename=\"file-{token}.bin\"\r\n\r\n\
             AQIDBA==\r\n\
             --mixed-boundary--\r\n"
        );
        let message = test_mail_message(&headers, &body);

        let (mime_headers, mime_body) = prepare_mime_entity(&message).unwrap();
        let assembled = format!(
            "{mime_headers}\r\n\r\n{}",
            String::from_utf8(mime_body).unwrap()
        );
        assert!(!assembled.contains(token));
        let parsed = MessageParser::default()
            .parse(assembled.as_bytes())
            .expect("transformed MIME should parse");

        let plain = parsed
            .body_text(0)
            .expect("plain alternative should survive");
        let html = parsed
            .body_html(0)
            .expect("HTML alternative should survive");
        assert!(plain.contains("Quoted reply"));
        assert!(!plain.contains(token));
        assert!(!plain.contains("list.example/unsubscribe"));
        assert!(html.contains("HTML reply"));
        assert!(!html.contains(token));
        assert!(!html.contains("unsubscribe?token"));

        let attachment = parsed
            .attachments()
            .next()
            .expect("attachment should survive");
        assert_eq!(attachment.contents(), &[1, 2, 3, 4]);
    }

    #[test]
    fn unmodified_mime_body_keeps_original_encoded_content() {
        let headers = serde_json::json!([
            ["MIME-Version", "1.0"],
            ["Content-Type", "multipart/mixed; boundary=\"b\""]
        ])
        .to_string();
        let body = concat!(
            "--b\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "Content-Transfer-Encoding: quoted-printable\r\n\r\n",
            "Plain =C3=BC\r\n",
            "--b\r\n",
            "Content-Type: application/octet-stream\r\n",
            "Content-Transfer-Encoding: base64\r\n\r\n",
            "AQIDBA==\r\n",
            "--b--\r\n"
        );
        let message = test_mail_message(&headers, body);

        let (_, transformed_body) = prepare_mime_entity(&message).unwrap();
        assert_eq!(String::from_utf8(transformed_body).unwrap(), body);
    }

    #[test]
    fn list_unsubscribe_token_is_added_after_body_redaction() {
        let own_token = "sub-8-abcdef0123456789abcdef0123456789";
        let generated_headers = format!(
            "From: list@example.test\r\nList-Unsubscribe: <https://example.test/unsub?token={own_token}>"
        );
        let result = assemble_raw_message(
            &generated_headers,
            "Content-Type: text/plain; charset=utf-8",
            b"Body without old token",
        )
        .unwrap();

        assert!(result.contains(own_token));
        assert!(result.contains("List-Unsubscribe:"));
    }

    #[test]
    fn test_build_unsubscribe_url_spacetimedb_route() {
        assert_eq!(
            build_unsubscribe_url(
                "http://localhost:3000/v1/database/kommunikation/route/mailing-list/unsubscribe",
                "sub-1-abc"
            ),
            "http://localhost:3000/v1/database/kommunikation/route/mailing-list/unsubscribe?token=sub-1-abc"
        );
        assert_eq!(
            build_unsubscribe_url(
                "http://localhost:3000/v1/database/kommunikation/route/mailing-list/unsubscribe/",
                "sub-1-abc"
            ),
            "http://localhost:3000/v1/database/kommunikation/route/mailing-list/unsubscribe?token=sub-1-abc"
        );
    }

    #[test]
    fn test_build_unsubscribe_url_frontend_base() {
        assert_eq!(
            build_unsubscribe_url("http://127.0.0.1:8080", "sub-1-abc"),
            "http://127.0.0.1:8080/?unsubscribe_token=sub-1-abc"
        );
        assert_eq!(
            build_unsubscribe_url("http://127.0.0.1:8080/", "sub-1-abc"),
            "http://127.0.0.1:8080/?unsubscribe_token=sub-1-abc"
        );
    }

    #[test]
    fn test_build_unsubscribe_url_with_existing_query() {
        assert_eq!(
            build_unsubscribe_url("https://admin.example.org/path?foo=bar", "sub-1-abc"),
            "https://admin.example.org/path?foo=bar&token=sub-1-abc"
        );
    }

    #[test]
    fn test_build_unsubscribe_url_with_custom_param() {
        assert_eq!(
            build_unsubscribe_url("https://admin.example.org/?unsubscribe=", "sub-1-abc"),
            "https://admin.example.org/?unsubscribe=sub-1-abc"
        );
    }
}
