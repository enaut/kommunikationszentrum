# Tables Reference

This page documents every SpacetimeDB table defined in the server module, grouped by functional area.

---

## User & Identity Management

### `account`

Stores user accounts synchronized from Django solawispielplatz.

```rust
#[spacetimedb::table(accessor = account, public)]
pub struct Account {
    #[primary_key]
    pub id: u64,             // Django membership number (mitgliedsnr)
    #[unique]
    pub identity: Identity,  // SpacetimeDB identity derived from OAuth issuer + mitgliedsnr
    pub name: String,
    #[index(btree)]
    pub email: String,
    pub is_active: bool,
    #[index(btree)]
    pub last_synced: Timestamp,
}
```

**Notes:**
- `identity` is computed via `Identity::from_claims(issuer_url, mitgliedsnr)` — it is
  deterministic given the same OAuth issuer and user ID.
- Direct queries are restricted by `ACCOUNT_VISIBILITY` (own row only for non-admins).
- Use the `visible_accounts` view for UI subscriptions.

---

### `admin_identities`

Tracks which SpacetimeDB identities have administrative privileges.

```rust
#[spacetimedb::table(accessor = admin_identities)]
pub struct AdminIdentity {
    #[primary_key]
    pub identity: Identity,
}
```

**Notes:**
- The module publisher's identity is automatically granted admin status during `init`.
- Managed via `register_admin_identity` / `unregister_admin_identity` reducers.
- `visible_admin_identities` view exposes this table to admins only.

---

### `webhook_tokens`

Stores hashed bearer tokens used by external systems (MTA, Django) to authenticate against HTTP routes.

```rust
#[spacetimedb::table(accessor = webhook_tokens)]
pub struct WebhookToken {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub token_hash: String,        // BLAKE3 hex hash — plaintext never stored
    pub label: String,             // Human-readable label for the token
    pub permissions: Vec<String>,  // e.g. ["mta-hook", "sync-user"]
    #[index(btree)]
    pub created_at: Timestamp,
    pub active: bool,
}
```

**Available permissions:**
- `mta-hook` — grants access to `POST /mta-hook`
- `sync-user` — grants access to `POST /user-sync`

---

## Mailing Lists & Subscriptions

### `message_categories`

Defines the available mailing list categories. Each category corresponds to one email address hosted by Stalwart MTA.

```rust
#[spacetimedb::table(accessor = message_categories, public)]
pub struct MessageCategory {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub name: String,
    #[unique]
    pub email_address: String,
    pub description: String,
    pub active: bool,
}
```

**Notes:**
- `email_address` is a `#[unique]` index and used for O(1) recipient lookups during RCPT/DATA stages.
- Categories are created via `add_message_category` (reducer) or `provision_message_category`
  (procedure, which also creates the Stalwart mailbox via JMAP).

---

### `subscriptions`

Links accounts to categories they are permitted to send to and will receive mail for.

```rust
#[spacetimedb::table(accessor = subscriptions, public)]
pub struct Subscription {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub subscriber_account_id: u64,   // → account.id
    #[index(btree)]
    pub subscriber_email: String,
    #[index(btree)]
    pub category_id: u64,             // → message_categories.id
    pub subscribed_at: Timestamp,
    pub active: bool,
}
```

**Notes:**
- An active subscription is required both to **receive** messages in that category and to
  **send** to it (unless the sender is an admin).
- Use `visible_subscriptions` or `active_subscriptions` views for client queries.

---

### `subscription_unsubscribe_tokens`

One-click unsubscribe tokens, one per subscription.

```rust
#[spacetimedb::table(accessor = subscription_unsubscribe_tokens, public)]
pub struct SubscriptionUnsubscribeToken {
    #[primary_key]
    pub token: String,             // "sub-{id}-{random128:032x}"
    #[unique]
    pub subscription_id: u64,      // → subscriptions.id
    #[index(btree)]
    pub created_at: Timestamp,
    pub active: bool,
    pub revoked_at: Timestamp,
}
```

**Usage:** The token is embedded in the `List-Unsubscribe-Post` header of every outgoing
delivery email. The recipient can POST to `/mailing-list/unsubscribe?token=…` to unsubscribe
without logging in.

---

## MTA Processing

### `mta_connection_log`

Per-connection event log for MTA hook stages (CONNECT, EHLO, MAIL, RCPT, AUTH).

```rust
#[spacetimedb::table(accessor = mta_connection_log)]
pub struct MtaConnectionLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub client_ip: String,   // May be "[REDACTED]" for privacy
    pub stage: String,       // "connect" | "ehlo" | "mail" | "rcpt" | "data" | "auth"
    pub action: String,      // "accept" | "reject"
    pub timestamp: Timestamp,
    pub details: String,
}
```

---

### `mta_message_log`

Per-message summary log for the DATA stage.

```rust
#[spacetimedb::table(accessor = mta_message_log)]
pub struct MtaMessageLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub from_address: String,
    pub to_addresses: String,      // JSON array of recipient addresses
    pub subject: String,           // Truncated to 100 chars
    pub message_size: u64,
    pub stage: String,
    pub action: String,            // "accept" | "quarantine"
    pub timestamp: Timestamp,
    pub queue_id: Option<String>,  // Stalwart queue ID
}
```

---

### `received_message`

Full message archive: one row per accepted message per category.

```rust
#[spacetimedb::table(accessor = received_message)]
pub struct ReceivedMessage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub queue_id: Option<String>,
    #[index(btree)]
    pub received_at: Timestamp,
    pub sender_account_id: Option<u64>,  // None for external senders
    pub sender_email: String,
    #[index(btree)]
    pub category_id: u64,
    pub category_email: String,
    pub subject: String,          // Capped at 500 chars
    pub from_header: String,
    pub date_header: Option<String>,
    pub message_id: Option<String>,
    pub reply_to: Option<String>,
    pub cc_header: Option<String>,
    pub headers_raw: String,      // JSON [[name, value], …]
    pub body_raw: String,         // Empty if message > 2 MB
    pub message_size: u64,
}
```

**Privacy:** Body is omitted for messages exceeding 2 MB. IP addresses are not stored here.

---

### `blocked_ips`

IP blocklist checked at the CONNECT stage.

```rust
#[spacetimedb::table(accessor = blocked_ips)]
pub struct BlockedIp {
    #[primary_key]
    pub ip: String,
    pub reason: String,
    pub blocked_at: Timestamp,
    pub active: bool,
}
```

---

## Delivery Pipeline

### `mail_ingress`

One record per accepted email per mailing list category. Represents the **ingress half** of the
delivery pipeline and is claimed by the sender daemon for fan-out processing.

```rust
#[spacetimedb::table(accessor = mail_ingress, public)]
pub struct MailIngress {
    #[primary_key]
    pub id: String,                         // "{queue_id}:{category_id}:{entropy}"
    #[index(btree)] pub queue_id: String,
    #[index(btree)] pub category_id: u64,
    #[index(btree)] pub state: String,      // See states below
    #[index(btree)] pub next_attempt_at: Timestamp,
    #[index(btree)] pub received_at: Timestamp,
    pub sender_account_id: Option<u64>,
    pub sender_email: String,
    pub category_email: String,
    pub subject: String,
    pub from_header: String,
    pub reply_to: Option<String>,
    pub date_header: Option<String>,
    pub message_id: Option<String>,
    pub cc_header: Option<String>,
    pub headers_raw: String,
    pub body_raw: String,
    pub message_size: u64,
    pub claim_owner: Option<Identity>,      // Sender daemon identity holding the lease
    pub claim_expires_at: Timestamp,
    pub attempt_count: u32,
    pub recipient_count: u32,
    pub delivery_count: u32,
    pub failed_delivery_count: u32,
    pub last_error: Option<String>,
    pub completed_at: Timestamp,
    pub updated_at: Timestamp,
}
```

**States:**

| State | Meaning |
|---|---|
| `pending` | Newly created, awaiting claim |
| `processing` | Claimed by a sender worker |
| `retry_scheduled` | Previous attempt failed; `next_attempt_at` is set |
| `completed` | Fan-out finished |
| `failed` | Exceeded maximum attempts (5) |

**Lease duration:** 10 minutes. Expired leases are re-claimable.

---

### `mail_deliveries`

One record per subscriber per ingress. Represents the **delivery half** of the pipeline — a
single SMTP submission to one recipient.

```rust
#[spacetimedb::table(accessor = mail_deliveries, public)]
pub struct MailDelivery {
    #[primary_key]
    pub id: String,                        // "{ingress_id}:{subscription_id}:{recipient_email}"
    #[index(btree)] pub ingress_id: String,
    #[index(btree)] pub category_id: u64,
    #[index(btree)] pub subscription_id: u64,
    #[index(btree)] pub recipient_email: String,
    #[index(btree)] pub state: String,     // See states below
    #[index(btree)] pub next_attempt_at: Timestamp,
    pub recipient_account_id: Option<u64>,
    pub list_email: String,
    pub list_name: String,
    pub original_sender_email: String,
    pub from_header: String,
    pub reply_to: String,
    pub subject: String,
    pub body_raw: String,
    pub headers_raw: String,
    pub raw_message: String,
    pub unsubscribe_token: String,
    pub claim_owner: Option<Identity>,
    pub claim_expires_at: Timestamp,
    pub attempt_count: u32,
    pub sent_at: Timestamp,
    pub last_error: Option<String>,
    pub smtp_status_code: Option<u16>,
    pub smtp_response: Option<String>,
    pub updated_at: Timestamp,
}
```

**States:**

| State | Meaning |
|---|---|
| `queued` | Waiting for initial SMTP attempt |
| `sending` | Claimed by a sender worker |
| `retry_scheduled` | Transient failure; back-off applied |
| `sent` | SMTP accepted |
| `failed` | Exceeded maximum attempts (5) |
| `bounced` | Permanent SMTP rejection |

**Lease duration:** 5 minutes.

**Retry back-off:**

| Attempt | Back-off |
|---|---|
| 1 | 30 s |
| 2 | 2 min |
| 3 | 10 min |
| 4 | 30 min |
| 5 | 60 min |
| 6+ | 12 h |

---

### `mail_delivery_events`

Immutable audit log of every state transition for a `MailDelivery`.

```rust
#[spacetimedb::table(accessor = mail_delivery_events, public)]
pub struct MailDeliveryEvent {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub delivery_id: String,
    #[index(btree)]
    pub occurred_at: Timestamp,
    pub event_type: String,              // State name at the time of the event
    pub attempt_no: u32,
    pub smtp_status_code: Option<u16>,
    pub smtp_response: Option<String>,
    pub error_kind: Option<String>,
    pub details: String,
    pub worker_identity: Option<Identity>,
}
```

---

## Views

SpacetimeDB **views** are server-side computed projections that determine what data a connecting
client can observe. All client subscriptions should target views rather than raw tables.

| View | Source table(s) | Admin | Regular user |
|---|---|---|---|
| `visible_accounts` | `account` | All rows | Own row only |
| `visible_admin_identities` | `admin_identities` | All rows | Empty |
| `visible_webhook_tokens` | `webhook_tokens` | All rows | Empty |
| `visible_subscriptions` | `subscriptions` | All rows | Own rows |
| `active_subscriptions` | `subscriptions` | Active only | Active only |
| `active_unsubscribe_tokens` | `subscription_unsubscribe_tokens` | Active only | Active only |
| `visible_messages` | `received_message` | All rows | Subscribed categories only |
| `sender_mail_ingress` | `mail_ingress` | All | All (sender daemon) |
| `sender_mail_deliveries` | `mail_deliveries` | All | All (sender daemon) |
