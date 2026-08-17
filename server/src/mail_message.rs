use spacetimedb::{Query, ReducerContext, Table, Timestamp, ViewContext};

use crate::account::is_admin_user;

/// One canonical record of a received email, written exactly once per
/// inbound message. Both `MailIngress` and per-recipient `MailDelivery*`
/// tables reference this by ID.
///
/// Private — clients never subscribe to this directly; use the
/// `sender_mail_messages` view below.
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

/// Insert one `MailMessage` row and return its auto-incremented `id`.
/// This is the single place where body truncation and subject capping live;
/// callers must NOT cap the subject before passing it here.
pub(crate) fn insert_mail_message(
    ctx: &ReducerContext,
    queue_id: Option<String>,
    sender_account_id: Option<u64>,
    sender_email: String,
    subject: String,
    from_header: String,
    reply_to: Option<String>,
    date_header: Option<String>,
    message_id: Option<String>,
    cc_header: Option<String>,
    headers_raw: String,
    body_raw: String,
    message_size: u64,
) -> u64 {
    ctx.db
        .mail_message()
        .insert(MailMessage {
            id: 0, // auto_inc — SpacetimeDB replaces this with the next value
            queue_id,
            received_at: ctx.timestamp,
            sender_account_id,
            sender_email,
            subject: subject.chars().take(500).collect(),
            from_header,
            reply_to,
            date_header,
            message_id,
            cc_header,
            headers_raw,
            body_raw,
            message_size,
        })
        .id
}

/// Exposes `MailMessage` rows to admin identities (i.e. the `sender` service).
/// Regular users get an empty result; they never need raw message content.
#[spacetimedb::view(accessor = sender_mail_messages, public)]
pub fn sender_mail_messages(ctx: &ViewContext) -> impl Query<MailMessage> {
    let is_admin = is_admin_user(ctx);
    ctx.from.mail_message().r#filter(move |_| is_admin)
}
