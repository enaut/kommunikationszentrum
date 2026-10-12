use mail_parser::MessageParser;
use stalwart_mta_hook_types::Request as MtaHookRequest;

/// Decode an RFC 2047 header value using mail_parser.
pub fn decode_rfc2047_header(header_name: &str, raw_value: &str) -> String {
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

/// Find the first header whose name (case-insensitive) matches `name` and return its trimmed value.
pub fn extract_header(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(n, _)| n.to_lowercase() == name)
        .map(|(_, v)| v.trim().to_string())
}

pub fn extract_subject_from_request(request: &MtaHookRequest) -> String {
    let raw = request
        .message
        .as_ref()
        .and_then(|m| extract_header(&m.headers, "subject"))
        .unwrap_or_else(|| "No subject".to_string());
    decode_rfc2047_header("Subject", &raw)
}

/// Parse a `To`-style header value into individual email addresses.
/// This is a permissive, heuristic parser that handles common forms like:
/// - "Alice <alice@example.com>, bob@example.com"
/// - "bob@example.com; carol@example.org"
pub fn parse_email_addresses(header: &str) -> Vec<String> {
    header
        .split(|c| c == ',' || c == ';')
        .filter_map(|part| {
            let s = part.trim();
            if s.is_empty() {
                return None;
            }
            // Prefer angle-bracket form: Name <addr@domain>
            if let Some(start) = s.find('<') {
                if let Some(end) = s.find('>') {
                    let addr = s[start + 1..end].trim();
                    if addr.contains('@') {
                        return Some(addr.to_string());
                    }
                }
            }
            // Otherwise, take the first whitespace-delimited token that contains '@'
            if let Some(tok) = s.split_whitespace().find(|t| t.contains('@')) {
                let addr = tok
                    .trim_matches(|c: char| c == '<' || c == '>' || c == '"' || c == '\'')
                    .trim()
                    .to_string();
                if addr.contains('@') {
                    return Some(addr);
                }
            }
            // Last-resort: if the whole part contains '@', return it cleaned
            if s.contains('@') {
                Some(
                    s.trim_matches(|c: char| c == '<' || c == '>' || c == '"' || c == '\'')
                        .trim()
                        .to_string(),
                )
            } else {
                None
            }
        })
        .collect()
}

pub fn extract_from_header_address(request: &MtaHookRequest) -> Option<String> {
    let message = request.message.as_ref()?;
    let from_raw = extract_header(&message.headers, "from")?;
    let decoded = decode_rfc2047_header("From", &from_raw);
    parse_email_addresses(&decoded).into_iter().next()
}

/// Extract the envelope sender address, if present, non-empty, and valid.
pub fn extract_envelope_from_address(request: &MtaHookRequest) -> Option<String> {
    request
        .envelope
        .as_ref()
        .map(|env| env.from.address.trim().to_string())
        .filter(|s| !s.is_empty() && s != "<>" && !s.eq_ignore_ascii_case("unknown"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_request(
        envelope_from: Option<&str>,
        from_header: Option<&str>,
        subject: Option<&str>,
    ) -> MtaHookRequest {
        let envelope_json = envelope_from.map(|ef| {
            serde_json::json!({
                "from": { "address": ef },
                "to": [{ "address": "topic@example.com" }]
            })
        });

        let mut headers = Vec::new();
        if let Some(fh) = from_header {
            headers.push(vec!["From", fh]);
        }
        if let Some(sub) = subject {
            headers.push(vec!["Subject", sub]);
        }

        let message_json = if from_header.is_some() || subject.is_some() {
            Some(serde_json::json!({
                "headers": headers,
                "server_headers": [],
                "contents": "Test content",
                "size": 100
            }))
        } else {
            None
        };

        let json = serde_json::json!({
            "context": {
                "stage": "data",
                "client": { "ip": "127.0.0.1", "port": 2525, "activeConnections": 1 },
                "server": { "name": "Stalwart", "port": 25, "ip": "127.0.0.1" },
                "protocol": { "version": 1 }
            },
            "envelope": envelope_json,
            "message": message_json
        });

        serde_json::from_value(json).expect("valid MtaHookRequest")
    }

    #[test]
    fn test_extract_from_header_address() {
        let req1 = make_test_request(
            Some("bounce@relay.example.com"),
            Some("Franz Dietrich <dietrich@teilgedanken.de>\r\n"),
            None,
        );
        assert_eq!(
            extract_from_header_address(&req1),
            Some("dietrich@teilgedanken.de".to_string())
        );

        let req2 = make_test_request(
            Some("bounce@relay.example.com"),
            Some("<dietrich@teilgedanken.de>"),
            None,
        );
        assert_eq!(
            extract_from_header_address(&req2),
            Some("dietrich@teilgedanken.de".to_string())
        );

        let req3 = make_test_request(
            Some("bounce@relay.example.com"),
            Some("dietrich@teilgedanken.de"),
            None,
        );
        assert_eq!(
            extract_from_header_address(&req3),
            Some("dietrich@teilgedanken.de".to_string())
        );

        let req4 = make_test_request(Some("bounce@relay.example.com"), None, None);
        assert_eq!(extract_from_header_address(&req4), None);
    }

    #[test]
    fn test_extract_envelope_from_address() {
        let req1 = make_test_request(
            Some("bounce.qs7b6n034mp4dy4=93cyiwtacik1=ae9wn8zzu7hmzd@em243835.teilgedanken.de"),
            None,
            None,
        );
        assert_eq!(
            extract_envelope_from_address(&req1),
            Some(
                "bounce.qs7b6n034mp4dy4=93cyiwtacik1=ae9wn8zzu7hmzd@em243835.teilgedanken.de"
                    .to_string()
            )
        );

        let req2 = make_test_request(Some("<>"), None, None);
        assert_eq!(extract_envelope_from_address(&req2), None);

        let req3 = make_test_request(Some("unknown"), None, None);
        assert_eq!(extract_envelope_from_address(&req3), None);

        let req4 = make_test_request(None, None, None);
        assert_eq!(extract_envelope_from_address(&req4), None);
    }

    #[test]
    fn test_parse_email_addresses() {
        assert_eq!(
            parse_email_addresses("Franz Dietrich <dietrich@teilgedanken.de>"),
            vec!["dietrich@teilgedanken.de"]
        );
        assert_eq!(
            parse_email_addresses("alice@example.com, bob@example.com"),
            vec!["alice@example.com", "bob@example.com"]
        );
        assert_eq!(
            parse_email_addresses("invalid; <valid@example.org>"),
            vec!["valid@example.org"]
        );
    }
}
