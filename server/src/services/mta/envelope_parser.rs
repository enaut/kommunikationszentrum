use stalwart_mta_hook_types::Request as MtaHookRequest;

/// Find the first header whose name (case-insensitive) matches `name` and return its trimmed value.
pub fn extract_header(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(n, _)| n.to_lowercase() == name)
        .map(|(_, v)| v.trim().to_string())
}

pub fn extract_subject_from_request(request: &MtaHookRequest) -> String {
    request
        .message
        .as_ref()
        .and_then(|m| extract_header(&m.headers, "subject"))
        .unwrap_or_else(|| "No subject".to_string())
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
