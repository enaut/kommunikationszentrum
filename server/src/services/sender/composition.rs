use std::{collections::HashMap, sync::OnceLock};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use mail_builder::{
    MessageBuilder,
    headers::{date::Date, raw::Raw},
};
use mail_parser::{MessageParser, PartKind};
use regex::Regex;

const DELIVERY_HEADER: &str = "X-SolaWi-Delivery-ID";

pub(super) fn compose_mail(
    from: &str,
    recipient: &str,
    topic_name: &str,
    subject: &str,
    message_id: &str,
    delivery_id: &str,
    unsubscribe_url: &str,
    headers_raw: &str,
    body_raw: &str,
    date_header: Option<&str>,
    date_timestamp_seconds: i64,
) -> Result<Vec<u8>, String> {
    if !valid_address(from) || !valid_address(recipient) {
        return Err("The sender or recipient address is not a valid mailbox address".to_string());
    }
    let (mime_headers, mime_body) = prepare_mime_entity(headers_raw, body_raw)?;
    let subject = list_subject(topic_name, subject);
    let mut builder = MessageBuilder::new()
        .from(from)
        .to(recipient)
        .reply_to(from)
        .subject(subject)
        .header("Message-ID", Raw::new(message_id.to_string()))
        .header(DELIVERY_HEADER, Raw::new(delivery_id.to_string()))
        .header(
            "List-Id",
            Raw::new(format!("{} <{}>", safe_header(topic_name), from)),
        )
        .header("List-Post", Raw::new(format!("<mailto:{from}>")))
        .header(
            "List-Unsubscribe",
            Raw::new(format!(
                "<mailto:{from}?subject=unsubscribe>, <{}>",
                safe_header(unsubscribe_url)
            )),
        )
        .header(
            "List-Unsubscribe-Post",
            Raw::new("List-Unsubscribe=One-Click"),
        )
        .header("Precedence", Raw::new("list"))
        .header("Sender", Raw::new(from.to_string()))
        .header("X-Mailing-List", Raw::new(safe_header(topic_name)))
        .header("X-BeenThere", Raw::new(from.to_string()));
    builder = if let Some(date) = date_header.filter(|date| !date.trim().is_empty()) {
        builder.header("Date", Raw::new(safe_header(date)))
    } else {
        builder.header("Date", Date::new(date_timestamp_seconds))
    };

    // mail-builder owns address/header formatting; the existing MIME entity is
    // appended byte-for-byte after its top-level MIME headers are restored.
    let formatted = builder
        .text_body(String::new())
        .write_to_string()
        .map_err(|error| format!("Could not compose RFC 5322 headers: {error}"))?;
    let generated_headers = formatted
        .split_once("\r\n\r\n")
        .map(|(headers, _)| headers)
        .ok_or_else(|| "mail-builder omitted the RFC 5322 header/body separator".to_string())?;
    let mut output = strip_generated_mime_headers(generated_headers);
    for line in mime_headers.split("\r\n").filter(|line| !line.is_empty()) {
        output.push_str(line);
        output.push_str("\r\n");
    }
    output.push_str("\r\n");
    let mut bytes = output.into_bytes();
    bytes.extend_from_slice(&mime_body);
    Ok(bytes)
}

fn valid_address(value: &str) -> bool {
    !value.contains(['\r', '\n', ' ', '\t'])
        && value.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        })
}

fn list_subject(topic_name: &str, subject: &str) -> String {
    let prefix = format!("[{topic_name}]: ");
    if subject
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
    {
        subject.to_string()
    } else {
        format!("{prefix}{subject}")
    }
}

fn safe_header(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)sub-[0-9]+-[0-9a-f]{32}").expect("valid token regex"))
}

fn unsubscribe_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)(?:https?://|mailto:|/)[^\s<>\"']*sub-[0-9]+-[0-9a-f]{32}[^\s<>\"']*"#)
            .expect("valid unsubscribe URL regex")
    })
}

fn unsubscribe_anchor_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?is)<a\b[^>]*\bhref\s*=\s*[\"'][^\"']*sub-[0-9]+-[0-9a-f]{32}[^\"']*[\"'][^>]*>.*?</a\s*>"#)
            .expect("valid unsubscribe anchor regex")
    })
}

fn clean_unsubscribe_content(value: &str) -> String {
    let without_links = unsubscribe_anchor_re().replace_all(value, "");
    let without_urls = unsubscribe_url_re().replace_all(&without_links, "");
    token_re().replace_all(&without_urls, "").into_owned()
}

fn prepare_mime_entity(headers_raw: &str, body_raw: &str) -> Result<(String, Vec<u8>), String> {
    let original_headers: Vec<(String, String)> = if headers_raw.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(headers_raw)
            .map_err(|error| format!("Malformed canonical headers: {error}"))?
    };
    let mut entity = String::new();
    let mut has_content_type = false;
    let mut has_mime_version = false;
    for (name, value) in original_headers {
        let lower_name = name.to_ascii_lowercase();
        if lower_name == "mime-version" || lower_name.starts_with("content-") {
            if lower_name == "content-type" {
                has_content_type = true;
            }
            if lower_name == "mime-version" {
                has_mime_version = true;
            }
            entity.push_str(&safe_header(&name));
            entity.push_str(": ");
            let value = if lower_name == "content-type" {
                value
            } else {
                clean_unsubscribe_content(&value)
            };
            entity.push_str(&safe_header(&value));
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
    entity.push_str(body_raw);

    let parsed = MessageParser::default()
        .parse(entity.as_bytes())
        .ok_or_else(|| "Could not parse the canonical MIME entity".to_string())?;
    let mut cleaned_text_parts = HashMap::new();
    for part in parsed.parts() {
        if !part.in_text_body() && !part.in_html_body() {
            continue;
        }
        let is_html = matches!(part.kind(), PartKind::Html);
        if !matches!(part.kind(), PartKind::Text | PartKind::Html) {
            continue;
        }
        let Some((text, problems)) = part.text_checked() else {
            continue;
        };
        let cleaned = clean_unsubscribe_content(&text);
        if cleaned != text {
            if !problems.is_empty() {
                return Err("Cannot safely clean an incorrectly encoded MIME text part".to_string());
            }
            cleaned_text_parts.insert(part.id(), (cleaned, is_html));
        }
    }

    let mut patches = Vec::new();
    for part in parsed.parts() {
        let id = part.id();
        let start = part.offset_header() as usize;
        let body_start = part.offset_body() as usize;
        let end = part.offset_end() as usize;
        if start > body_start || body_start > end || end > entity.len() {
            return Err("MIME parser returned invalid part offsets".to_string());
        }
        let cleaned_body = cleaned_text_parts.get(&id);
        let raw_headers = &entity.as_bytes()[start..body_start];
        let fields = parse_header_fields(
            std::str::from_utf8(raw_headers).map_err(|error| error.to_string())?,
        );
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
        let mut replacement = new_headers.into_bytes();
        replacement.extend_from_slice(b"\r\n");
        patches.push((start, body_start, replacement));
        if let Some((cleaned, _)) = cleaned_body {
            patches.push((body_start, end, base64_mime_body(cleaned.as_bytes())));
        }
    }
    let mut entity_bytes = entity.into_bytes();
    patches.sort_by(|left, right| right.0.cmp(&left.0));
    for (start, end, replacement) in patches {
        entity_bytes.splice(start..end, replacement);
    }
    let entity = String::from_utf8(entity_bytes).map_err(|error| error.to_string())?;
    let (mime_headers, mime_body) = entity
        .split_once("\r\n\r\n")
        .ok_or_else(|| "Prepared MIME entity has no header/body separator".to_string())?;
    Ok((mime_headers.to_string(), mime_body.as_bytes().to_vec()))
}

fn rewrite_mime_part_headers(
    raw_headers: &[u8],
    rewrite_body: bool,
    is_html: bool,
) -> Result<String, String> {
    let raw_headers = std::str::from_utf8(raw_headers).map_err(|error| error.to_string())?;
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
                result.push_str(&safe_header(&value));
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
                result.push_str(&safe_header(&value));
                result.push_str("\r\n");
                saw_content_type = true;
            }
        } else {
            result.push_str(&name);
            result.push_str(": ");
            result.push_str(&safe_header(&clean_unsubscribe_content(&value)));
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
    result.trim_end_matches("\r\n").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_old_unsubscribe_tokens_without_losing_multipart_attachments() {
        let token = "sub-17-0123456789abcdef0123456789abcdef";
        let headers = serde_json::json!([
            ["MIME-Version", "1.0"],
            ["Content-Type", "multipart/mixed; boundary=\"mixed\""]
        ])
        .to_string();
        let body = format!(
            "--mixed\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello https://list.example/unsubscribe?token={token}\r\n--mixed\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\nContent-Disposition: attachment; filename=\"x.bin\"\r\n\r\nAAECAw==\r\n--mixed--\r\n"
        );
        let composed = compose_mail(
            "list@example.org",
            "member@example.org",
            "List",
            "Subject",
            "<id@example.org>",
            "delivery-1",
            "https://list.example/unsubscribe?token=sub-1-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            &headers,
            &body,
            None,
            1_700_000_000,
        )
        .unwrap();
        let parsed = MessageParser::default().parse(&composed).unwrap();
        assert!(String::from_utf8_lossy(&composed).contains("X-SolaWi-Delivery-ID: delivery-1"));
        assert!(parsed.attachments().next().is_some());
        assert!(!String::from_utf8_lossy(&composed).contains(token));
    }

    #[test]
    fn rejects_header_injection_in_addresses() {
        assert!(
            compose_mail(
                "list@example.org",
                "victim@example.org\r\nBcc: other@example.org",
                "List",
                "Subject",
                "<id@example.org>",
                "delivery",
                "",
                "[]",
                "body",
                None,
                1_700_000_000
            )
            .is_err()
        );
    }
}
