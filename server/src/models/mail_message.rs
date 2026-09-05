use spacetimedb::Timestamp;

/// One canonical record of a received email, written exactly once per
/// inbound message. Both `MailIngress` and per-recipient `MailDeliveryMessage`
/// tables reference this by ID.
#[derive(Clone)]
#[spacetimedb::table(accessor = mail_message)]
pub struct MailMessage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,

    // --- delivery routing (needed by ingress fan-out) ---
    pub queue_id: Option<String>,
    /// When this row was inserted.
    pub received_at: Timestamp,
    /// FK → Account.id; None when the sender is not a known member.
    pub sender_account_id: Option<u64>,
    /// Raw envelope sender address.
    pub sender_email: String,

    // --- original RFC 5322 headers ---
    /// Parsed Subject header, capped at 500 chars on insert.
    pub subject: String,
    /// Raw From header value.
    pub from_header: String,
    pub reply_to: Option<String>,
    pub date_header: Option<String>,
    pub message_id: Option<String>,
    pub cc_header: Option<String>,

    // --- raw content ---
    /// JSON array of [name, value] pairs (original + server-added headers).
    pub headers_raw: String,
    /// Full body; empty string when message exceeds 2 MB.
    pub body_raw: String,
    pub message_size: u64,
}
