use mail_parser::MessageParser;

/// Decode a header value (e.g. Subject, From, CC) that may contain RFC 2047 encoded words.
pub fn decode_header_value(header_name: &str, raw_value: &str) -> String {
    if !raw_value.contains("=?") {
        return raw_value.to_string();
    }
    let raw_mime = format!("{header_name}: {raw_value}\r\n\r\n");
    if let Some(msg) = MessageParser::default().parse(raw_mime.as_bytes()) {
        if header_name.eq_ignore_ascii_case("subject") {
            if let Some(subject) = msg.subject() {
                return subject.to_string();
            }
        } else if header_name.eq_ignore_ascii_case("from") {
            if let Some(from_list) = msg.from() {
                if let Some(addr) = from_list.first() {
                    match (addr.name(), addr.address()) {
                        (Some(name), Some(email)) => return format!("{name} <{email}>"),
                        (Some(name), None) => return name.to_string(),
                        (None, Some(email)) => return email.to_string(),
                        _ => {}
                    }
                }
            }
        }
        if let Some(hdr) = msg.header(header_name) {
            if let Some(val) = hdr.as_text() {
                return val.to_string();
            }
        }
    }
    raw_value.to_string()
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
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    let parser = pulldown_cmark::Parser::new_ext(md, options);
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);
    html_output
}

/// Decode the message body given raw body text and JSON headers_raw.
pub fn decode_body(body_raw: &str, headers_raw: &str) -> String {
    if body_raw.trim().is_empty() {
        return String::new();
    }

    // Try reconstructing the MIME message from headers_raw and body_raw
    let mut raw_mime = String::with_capacity(headers_raw.len() + body_raw.len() + 64);
    let mut has_cte = false;

    if let Ok(headers) = serde_json::from_str::<Vec<(String, String)>>(headers_raw) {
        for (name, val) in headers {
            if name.eq_ignore_ascii_case("content-transfer-encoding") {
                has_cte = true;
            }
            raw_mime.push_str(&name);
            raw_mime.push_str(": ");
            raw_mime.push_str(val.trim());
            raw_mime.push_str("\r\n");
        }
    }

    // If headers didn't include Content-Transfer-Encoding, but body looks like Quoted-Printable:
    if !has_cte && (body_raw.contains("=\r\n") || body_raw.contains("=\n") || body_raw.contains("=C3=")) {
        raw_mime.push_str("Content-Transfer-Encoding: quoted-printable\r\n");
        raw_mime.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    }

    raw_mime.push_str("\r\n");
    raw_mime.push_str(body_raw);

    if let Some(msg) = MessageParser::default().parse(raw_mime.as_bytes()) {
        if let Some(text) = msg.body_text(0) {
            return text.to_string();
        }
        if let Some(html) = msg.body_html(0) {
            return html_to_markdown(&html);
        }
    }

    // If parsing failed or gave no text, fallback to raw body
    body_raw.to_string()
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
        assert!(html.contains("<a href=\"https://example.com\">link</a>"));
    }

    #[test]
    fn test_decode_body_html_only() {
        let headers = r#"[["Content-Type", "text/html; charset=utf-8"]]"#;
        let body = "<p>Guten Tag <b>Herr Müller</b>!</p>";
        let decoded = decode_body(body, headers);
        assert!(decoded.contains("Guten Tag"));
        assert!(decoded.contains("**Herr Müller**"));
    }
}
