use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use base64::Engine as _;
use mail_parser::{MessageParser, MimeHeaders, PartType};
use regex::Regex;

/// Decode a header value (e.g. Subject, From, CC) that may contain RFC 2047 encoded words.
pub fn decode_header_value(header_name: &str, raw_value: &str) -> String {
    let decoded = if !raw_value.contains("=?") {
        raw_value.to_string()
    } else {
        let raw_mime = format!("{header_name}: {raw_value}\r\n\r\n");
        MessageParser::default()
            .parse(raw_mime.as_bytes())
            .and_then(|msg| {
                if header_name.eq_ignore_ascii_case("subject") {
                    if let Some(subject) = msg.subject() {
                        return Some(subject.to_string());
                    }
                } else if header_name.eq_ignore_ascii_case("from") {
                    if let Some(from_list) = msg.from() {
                        if let Some(addr) = from_list.first() {
                            match (addr.name(), addr.address()) {
                                (Some(name), Some(email)) => {
                                    return Some(format!("{name} <{email}>"));
                                }
                                (Some(name), None) => return Some(name.to_string()),
                                (None, Some(email)) => return Some(email.to_string()),
                                _ => {}
                            }
                        }
                    }
                }
                msg.header(header_name)
                    .and_then(|header| header.as_text())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| raw_value.to_string())
    };

    redact_unsubscribe_tokens(&decoded)
}

/// Remove project unsubscribe tokens wherever they occur in displayed content.
///
/// This deliberately handles text and markup identically so tokens in quoted text,
/// HTML attributes, and link destinations are redacted before rendering.
fn unsubscribe_token_re() -> &'static Regex {
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

pub fn redact_unsubscribe_tokens(input: &str) -> String {
    let without_links = unsubscribe_anchor_re().replace_all(input, "");
    let without_urls = unsubscribe_url_re().replace_all(&without_links, "");
    unsubscribe_token_re()
        .replace_all(&without_urls, "")
        .into_owned()
}

/// Return a source view with tokens removed, decoding text parts when a token is
/// hidden by a MIME transfer encoding such as base64.
pub fn redact_raw_body(body_raw: &str, headers_raw: &str) -> String {
    let redacted_raw = redact_unsubscribe_tokens(body_raw);
    let raw_mime = reconstruct_mime(body_raw, headers_raw);
    let Some(msg) = MessageParser::default().parse(raw_mime.as_bytes()) else {
        return redacted_raw;
    };

    let mut found_token = false;
    let mut decoded_parts = Vec::new();
    for id in msg.text_body.iter().chain(msg.html_body.iter()) {
        let Some(part) = msg.parts.get(*id as usize) else {
            continue;
        };
        let text = match &part.body {
            PartType::Text(text) | PartType::Html(text) => text.as_ref(),
            _ => continue,
        };
        let redacted = redact_unsubscribe_tokens(text);
        if redacted != text {
            found_token = true;
        }
        decoded_parts.push(redacted);
    }

    if found_token {
        decoded_parts.join("\n\n--- MIME body part ---\n\n")
    } else {
        redacted_raw
    }
}

/// Convert an HTML body into Markdown (lossy) using `htmd`.
pub fn html_to_markdown(html: &str) -> String {
    let converter = htmd::HtmlToMarkdown::builder()
        .skip_tags(vec!["head", "style", "script", "noscript"])
        .build();
    match converter.convert(html) {
        Ok(md) => md.trim().to_string(),
        Err(_) => html.to_string(),
    }
}

/// Render Markdown (or plain text) into sanitized HTML for display in the UI.
pub fn render_markdown_to_html(md: &str) -> String {
    let redacted = redact_unsubscribe_tokens(md);
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    let parser = pulldown_cmark::Parser::new_ext(&redacted, options);
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);
    sanitize_html(&html_output, &HashMap::new())
}

fn html_body_part(msg: &mail_parser::Message<'_>) -> Option<String> {
    msg.html_body
        .iter()
        .find_map(|id| match &msg.parts.get(*id as usize)?.body {
            PartType::Html(html) => Some(html.to_string()),
            _ => None,
        })
}

/// Decode the preferred MIME body: HTML when available, otherwise plain text.
pub fn decode_body(body_raw: &str, headers_raw: &str) -> String {
    if body_raw.trim().is_empty() {
        return String::new();
    }

    let raw_mime = reconstruct_mime(body_raw, headers_raw);
    if let Some(msg) = MessageParser::default().parse(raw_mime.as_bytes()) {
        if let Some(html) = html_body_part(&msg) {
            return redact_unsubscribe_tokens(&html);
        }
        if let Some(text) = msg.body_text(0) {
            return redact_unsubscribe_tokens(&text);
        }
    }

    redact_unsubscribe_tokens(body_raw)
}

/// Decode and safely render a MIME message body, resolving only matching local CID images.
pub fn render_message_body(body_raw: &str, headers_raw: &str) -> String {
    if body_raw.trim().is_empty() {
        return String::new();
    }

    let raw_mime = reconstruct_mime(body_raw, headers_raw);
    if let Some(msg) = MessageParser::default().parse(raw_mime.as_bytes()) {
        if let Some(html) = html_body_part(&msg) {
            let redacted_html = redact_unsubscribe_tokens(&html);
            let inline_images = inline_cid_images(&msg);
            return sanitize_html(&redacted_html, &inline_images);
        }
        if let Some(text) = msg.body_text(0) {
            return render_markdown_to_html(&text);
        }
    }

    render_markdown_to_html(body_raw)
}

fn reconstruct_mime(body_raw: &str, headers_raw: &str) -> String {
    let mut raw_mime = String::with_capacity(headers_raw.len() + body_raw.len() + 64);
    let mut has_cte = false;
    let mut has_content_type = false;
    let mut is_multipart = false;

    if let Ok(headers) = serde_json::from_str::<Vec<(String, String)>>(headers_raw) {
        for (name, val) in headers {
            if name.eq_ignore_ascii_case("content-transfer-encoding") {
                has_cte = true;
            } else if name.eq_ignore_ascii_case("content-type") {
                has_content_type = true;
                is_multipart = val
                    .trim_start()
                    .get(..10)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("multipart/"));
            }
            raw_mime.push_str(&name);
            raw_mime.push_str(": ");
            raw_mime.push_str(val.trim());
            raw_mime.push_str("\r\n");
        }
    }

    if !has_cte
        && !is_multipart
        && (body_raw.contains("=\r\n") || body_raw.contains("=\n") || body_raw.contains("=C3="))
    {
        raw_mime.push_str("Content-Transfer-Encoding: quoted-printable\r\n");
        if !has_content_type {
            raw_mime.push_str("Content-Type: text/plain; charset=utf-8\r\n");
        }
    }

    raw_mime.push_str("\r\n");
    raw_mime.push_str(body_raw);
    raw_mime
}

fn normalize_content_id(value: &str) -> Option<String> {
    let value = value.trim();
    let id = if value
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("cid:"))
    {
        &value[4..]
    } else {
        value
    };
    let decoded = urlencoding::decode(id).ok()?;
    let normalized = decoded
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim();
    (!normalized.is_empty()).then(|| normalized.to_string())
}

fn inline_cid_images(msg: &mail_parser::Message<'_>) -> HashMap<String, String> {
    let mut images = HashMap::new();

    for part in msg.attachments() {
        let (Some(content_id), Some(content_type)) = (part.content_id(), part.content_type())
        else {
            continue;
        };
        if !content_type.ctype().eq_ignore_ascii_case("image") {
            continue;
        }
        if !matches!(&part.body, PartType::Binary(_) | PartType::InlineBinary(_)) {
            continue;
        }

        let Some(subtype) = content_type.subtype() else {
            continue;
        };
        if !matches!(
            subtype.to_ascii_lowercase().as_str(),
            "png" | "jpeg" | "gif" | "webp" | "avif" | "bmp"
        ) {
            continue;
        }
        let Some(content_id) = normalize_content_id(content_id) else {
            continue;
        };
        let media_type = format!("image/{}", subtype.to_ascii_lowercase());
        let data_url = format!(
            "data:{media_type};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(part.contents())
        );
        images.entry(content_id).or_insert(data_url);
    }

    images
}

fn sanitize_html(html: &str, inline_images: &HashMap<String, String>) -> String {
    let tags: HashSet<&str> = [
        "a",
        "abbr",
        "b",
        "blockquote",
        "br",
        "code",
        "dd",
        "del",
        "div",
        "dl",
        "dt",
        "em",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "i",
        "img",
        "li",
        "ol",
        "p",
        "pre",
        "s",
        "small",
        "span",
        "strong",
        "sub",
        "sup",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "tr",
        "u",
        "ul",
    ]
    .into_iter()
    .collect();
    let attributes = HashMap::from([
        ("a", HashSet::from(["href", "title"])),
        ("img", HashSet::from(["src", "alt"])),
        ("td", HashSet::from(["colspan", "rowspan"])),
        ("th", HashSet::from(["colspan", "rowspan"])),
    ]);
    let schemes: HashSet<&str> = ["http", "https", "mailto", "data", "cid"]
        .into_iter()
        .collect();
    let inline_images = inline_images.clone();
    let mut builder = ammonia::Builder::default();
    builder
        .tags(tags)
        .tag_attributes(attributes)
        .generic_attributes(HashSet::new())
        .url_schemes(schemes)
        .attribute_filter(move |element, attribute, value| {
            if element == "a" && attribute == "href" && unsubscribe_token_re().is_match(value) {
                return None;
            }

            if element == "img" && attribute == "src" {
                if let Some(content_id) = normalize_content_id(value) {
                    if let Some(data_url) = inline_images.get(&content_id) {
                        return Some(Cow::Owned(data_url.clone()));
                    }
                }
                return None;
            }

            if value
                .get(..5)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
                || value
                    .get(..4)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("cid:"))
            {
                return None;
            }
            Some(Cow::Owned(value.to_string()))
        });

    let cleaned = builder.clean(html).to_string();
    redact_unsubscribe_tokens(&cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_markdown_basic() {
        let html = "<h1>Title</h1><p>Hello <b>world</b> and <i>italic</i>.</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("# Title") || md.contains("Title\n="));
        assert!(md.contains("**world**"));
        assert!(md.contains("*italic*"));
    }

    #[test]
    fn test_html_to_markdown_strips_styles_and_scripts() {
        let html = "<html><head><style>body { color: red; }</style></head><body><p>Visible text</p><script>alert('xss');</script></body></html>";
        let md = html_to_markdown(html);
        assert!(!md.contains("color: red"));
        assert!(!md.contains("alert"));
        assert!(md.contains("Visible text"));
    }

    #[test]
    fn test_html_to_markdown_links_and_lists() {
        let html = "<p>Check <a href=\"https://solawi.de\">this link</a></p><ul><li>First item</li><li>Second item</li></ul>";
        let md = html_to_markdown(html);
        assert!(md.contains("[this link](https://solawi.de)"));
        assert!(md.contains("First item"));
        assert!(md.contains("Second item"));
    }

    #[test]
    fn test_render_markdown_to_html() {
        let md = "# Welcome\n\nThis is **bold** and a [link](https://example.com).";
        let html = render_markdown_to_html(md);
        assert!(html.contains("<h1>Welcome</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("href=\"https://example.com\""));
    }

    #[test]
    fn multipart_html_is_preferred_over_plain_text() {
        let headers = r#"[["Content-Type", "multipart/alternative; boundary=\"mail-boundary\""]]"#;
        let body = concat!(
            "--mail-boundary\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n\r\n",
            "plain body\r\n",
            "--mail-boundary\r\n",
            "Content-Type: text/html; charset=utf-8\r\n\r\n",
            "<p>HTML body</p>\r\n",
            "--mail-boundary--\r\n"
        );
        let decoded = decode_body(body, headers);
        assert!(decoded.contains("<p>HTML body</p>"));
        assert!(!decoded.contains("plain body"));
    }

    #[test]
    fn plain_text_is_used_when_html_is_missing() {
        let headers = r#"[["Content-Type", "text/plain; charset=utf-8"]]"#;
        let decoded = decode_body("plain body", headers);
        assert_eq!(decoded, "plain body");
    }

    #[test]
    fn cid_image_is_resolved_only_from_matching_image_attachment() {
        let headers = r#"[["Content-Type", "multipart/related; boundary=\"related-boundary\""]]"#;
        let body = concat!(
            "--related-boundary\r\n",
            "Content-Type: text/html; charset=utf-8\r\n\r\n",
            "<p>Inline</p><img src=\"cid:local-image\"><img src=\"cid:missing-image\">",
            "<img src=\"cid:not-an-image\"><img src=\"https://remote.example/image.png\">\r\n",
            "--related-boundary\r\n",
            "Content-Type: image/png\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "Content-ID: <local-image>\r\n",
            "Content-Disposition: inline\r\n\r\n",
            "AQID\r\n",
            "--related-boundary\r\n",
            "Content-Type: application/pdf\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "Content-ID: <not-an-image>\r\n",
            "Content-Disposition: inline\r\n\r\n",
            "AQID\r\n",
            "--related-boundary--\r\n"
        );
        let rendered = render_message_body(body, headers);
        assert!(rendered.contains("src=\"data:image/png;base64,AQID\""));
        assert!(!rendered.contains("cid:local-image"));
        assert!(!rendered.contains("missing-image"));
        assert!(!rendered.contains("not-an-image"));
        assert!(!rendered.contains("remote.example"));
    }

    #[test]
    fn unsubscribe_tokens_are_removed_from_text_markup_and_headers() {
        let token = "sub-42-0123456789abcdef0123456789abcdef";
        let text = format!("Quoted: > {token}\nLink: /unsubscribe/{token}");
        assert_eq!(redact_unsubscribe_tokens(&text), "Quoted: > \nLink: ");
        assert_eq!(
            decode_header_value("Subject", &format!("Topic {token}")),
            "Topic "
        );

        let headers = r#"[["Content-Type", "text/html; charset=utf-8"]]"#;
        let html = format!(
            "<p>quoted {token}</p><a href=\"https://example.test/{token}\">unsubscribe</a>"
        );
        let rendered = render_message_body(&html, headers);
        assert!(!rendered.contains(token));
        assert!(!rendered.contains("https://example.test/"));
        assert!(!rendered.contains("unsubscribe"));
    }

    #[test]
    fn raw_view_decodes_and_redacts_tokens_hidden_by_base64() {
        let token = "sub-42-0123456789abcdef0123456789abcdef";
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(format!("<p>{token}</p>").as_bytes());
        let headers = r#"[["Content-Type", "text/html; charset=utf-8"], ["Content-Transfer-Encoding", "base64"]]"#;

        let redacted = redact_raw_body(&encoded, headers);

        assert!(redacted.contains("<p></p>"));
        assert!(!redacted.contains(token));
        assert!(!redacted.contains(&encoded));
    }

    #[test]
    fn sanitizer_removes_scripts_events_and_remote_resources_but_keeps_links() {
        let html = concat!(
            "<script>alert('xss')</script><style>@import url(https://evil.test/x.css);</style>",
            "<p onclick=\"alert(1)\" style=\"background:url(https://evil.test/x)\">Visible</p>",
            "<img src=\"https://remote.test/pixel.png\" srcset=\"https://remote.test/a 2x\" ",
            "background=\"https://remote.test/bg\">",
            "<iframe src=\"https://evil.test\">frame text</iframe>",
            "<form action=\"https://evil.test\"><input></form>",
            "<a href=\"javascript:alert(1)\">bad link</a>",
            "<a href=\"https://example.test/page\">ordinary link</a>",
            "<a href=\"mailto:person@example.test\">email link</a>"
        );
        let clean = sanitize_html(html, &HashMap::new());
        assert!(!clean.contains("alert('xss')"));
        assert!(!clean.contains("@import"));
        assert!(!clean.contains("onclick"));
        assert!(!clean.contains("style="));
        assert!(!clean.contains("evil.test"));
        assert!(!clean.contains("remote.test"));
        assert!(!clean.contains("javascript:"));
        assert!(clean.contains("Visible"));
        assert!(clean.contains("href=\"https://example.test/page\""));
        assert!(clean.contains("href=\"mailto:person@example.test\""));
        assert!(!clean.contains("<form"));
        assert!(!clean.contains("<iframe"));
    }

    #[test]
    fn arbitrary_data_urls_are_not_allowed() {
        let clean = sanitize_html(
            "<img src=\"data:image/png;base64,arbitrary\"><a href=\"data:text/html,evil\">link</a>",
            &HashMap::new(),
        );
        assert!(!clean.contains("data:"));
    }
}
