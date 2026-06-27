# Reducers & Procedures Reference

Reducers are atomic, transactional business-logic functions that modify the SpacetimeDB database.
Procedures extend reducers with the ability to perform external I/O (HTTP calls). Both are callable
by the Admin UI, the sender daemon, and via the `spacetime call` CLI.

---

## Lifecycle Reducers

These are invoked automatically by SpacetimeDB and cannot be called manually.

### `init`

```rust
#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext)
```

Called once when the module is first published. Seeds the publisher's identity into
`admin_identities` if it isn't already present, ensuring at least one admin always exists.

---

### `identity_connected`

```rust
#[spacetimedb::reducer(client_connected)]
pub fn identity_connected(ctx: &ReducerContext)
```

Called each time a WebSocket client connects. Logs the connecting identity. Can be extended
to perform connection-time authorization checks.

---

### `identity_disconnected`

```rust
#[spacetimedb::reducer(client_disconnected)]
pub fn identity_disconnected(_ctx: &ReducerContext)
```

Called each time a WebSocket client disconnects. Currently a no-op.

---

## Account & Identity Management

### `register_admin_identity`

```rust
pub fn register_admin_identity(ctx: &ReducerContext, identity_hex: String) -> Result<(), String>
```

Grants admin status to the identity identified by `identity_hex` (64-character hex).
Only existing admins may call this. Idempotent — calling it multiple times is safe.

---

### `unregister_admin_identity`

```rust
pub fn unregister_admin_identity(ctx: &ReducerContext, identity_hex: String) -> Result<(), String>
```

Removes admin status from the given identity. Only existing admins may call this.

---

### `sync_user`

```rust
pub fn sync_user(ctx: &ReducerContext, action: String, user_data: String) -> Result<(), String>
```

Synchronizes a single user account from Django. Only admin identities may call this directly
(the HTTP `/user-sync` endpoint wraps it with token authentication).

**Parameters:**
- `action` — `"upsert"` or `"delete"`
- `user_data` — JSON-serialized `UserSyncData`:
  ```json
  {
    "mitgliedsnr": 42,
    "name": "Alice Example",
    "email": "alice@example.org",
    "is_active": true,
    "is_admin": false,
    "updated_at": "2026-01-01T00:00:00Z",
    "identity_hex": null
  }
  ```

**Upsert behaviour:**
1. Computes `Identity::from_claims(issuer_url, mitgliedsnr)`.
2. If the account exists, updates it in place. If not, inserts it.
3. Syncs `admin_identities`: adds if `is_admin=true`, removes if `is_admin=false`.

**Delete behaviour:**
1. Deletes the `account` row.
2. Removes from `admin_identities` if present.

---

### `create_webhook_token`

```rust
pub fn create_webhook_token(
    ctx: &ReducerContext,
    token_hash: String,
    label: String,
    permissions: Vec<String>,
) -> Result<(), String>
```

Stores a new webhook token. The **caller must hash the plaintext token with BLAKE3** before
calling this reducer — the plaintext never enters the module.

Only admins may call this.

---

### `revoke_webhook_token`

```rust
pub fn revoke_webhook_token(ctx: &ReducerContext, token_hash: String) -> Result<(), String>
```

Permanently deletes a webhook token row. Only admins may call this.

---

## Mailing List Management

### `add_message_category`

```rust
pub fn add_message_category(
    ctx: &ReducerContext,
    name: String,
    email_address: String,
    description: String,
) -> Result<(), String>
```

Creates a new `MessageCategory` with `active: true`. Only admins. Does **not** provision the
Stalwart mailbox — use `provision_message_category` for that.

---

### `remove_message_category`

```rust
pub fn remove_message_category(ctx: &ReducerContext, category_id: u64) -> Result<(), String>
```

Deletes the `MessageCategory` row. Returns an error if the category does not exist. Only admins.

---

### `provision_message_category` _(Procedure)_

```rust
#[spacetimedb::procedure]
pub fn provision_message_category(
    ctx: &mut ProcedureContext,
    name: String,
    email_address: String,
    description: String,
) -> Result<(), String>
```

A **procedure** (not a plain reducer) that:
1. Checks admin authorization.
2. Verifies no category with that email already exists.
3. POSTs a JMAP `x:Account/set` request to Stalwart to create the mailbox.
4. On success, inserts the `MessageCategory` row inside a transaction.

**Compile-time configuration** (set via environment variables at module build time):
- `STALWART_JMAP_URL` — base URL of the Stalwart JMAP endpoint
- `STALWART_ADMIN_TOKEN` — admin bearer token for JMAP requests

The procedure fails atomically: if the Stalwart request fails, no category row is inserted.

---

## Subscription Management

### `add_subscription`

```rust
pub fn add_subscription(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    subscriber_email: String,
    category_id: u64,
) -> Result<(), String>
```

Creates or re-activates a `Subscription`. Callers may subscribe themselves or, if admin, any account.

Also calls `upsert_subscription_unsubscribe_token` to ensure a valid one-click unsubscribe
token exists for the subscription.

---

### `remove_subscription`

```rust
pub fn remove_subscription(ctx: &ReducerContext, subscription_id: u64) -> Result<(), String>
```

Soft-deletes the subscription (`active = false`) and deactivates the corresponding
unsubscribe token. Callers may remove their own subscriptions or, if admin, any subscription.

---

### `ensure_subscription_unsubscribe_token`

```rust
pub fn ensure_subscription_unsubscribe_token(
    ctx: &ReducerContext,
    subscription_id: u64,
) -> Result<(), String>
```

Idempotently creates an unsubscribe token for an existing subscription if one doesn't exist.
Useful for back-filling tokens after a migration.

---

## MTA Hook Processing

### `handle_mta_hook`

```rust
pub fn handle_mta_hook(ctx: &ReducerContext, hook_data: String) -> Result<(), String>
```

> **Note:** In practice this reducer is only used by the sender daemon or during local
> testing. The production HTTP route (`/mta-hook`) calls the stage handlers directly inside a
> transaction for synchronous accept/reject responses.

Parses the Stalwart `MtaHookRequest` JSON and dispatches to the appropriate stage handler:

| Stage | Handler | Decision logic |
|---|---|---|
| `Connect` | `handle_connect_stage` | Checks `blocked_ips` |
| `Ehlo` | `handle_ehlo_stage` | Validates HELO string |
| `Mail` | `handle_mail_stage` | Validates `MAIL FROM` address |
| `Rcpt` | `handle_rcpt_stage` | Checks recipient against `message_categories` |
| `Data` | `handle_data_stage` | Full subscription check + message persistence |
| `Auth` | `handle_auth_stage` | Accept-all |

---

### `dump_mta_logs_to_server_logs`

```rust
pub fn dump_mta_logs_to_server_logs(ctx: &ReducerContext)
```

Debug utility. Prints all `mta_connection_log` and `mta_message_log` rows to the SpacetimeDB
server log. Useful during development and incident investigation.

```bash
spacetime call kommunikationszentrum dump_mta_logs_to_server_logs
```

---

## Delivery Pipeline

### `claim_next_mail_ingress`

```rust
pub fn claim_next_mail_ingress(ctx: &ReducerContext) -> Result<(), String>
```

Called by the sender daemon to atomically claim the next claimable `MailIngress` record.
A record is claimable when:
- State is `pending` or `retry_scheduled`
- `next_attempt_at <= now`
- No owner, or owner's lease has expired

Sets state → `processing`, increments `attempt_count`, sets `claim_owner` and a 10-minute
`claim_expires_at`.

---

### `complete_mail_ingress`

```rust
pub fn complete_mail_ingress(
    ctx: &ReducerContext,
    ingress_id: String,
    delivery_count: u32,
    failed_delivery_count: u32,
) -> Result<(), String>
```

Marks a claimed ingress as `completed`. Only the claim owner may call this.

---

### `retry_mail_ingress`

```rust
pub fn retry_mail_ingress(
    ctx: &ReducerContext,
    ingress_id: String,
    error: String,
) -> Result<(), String>
```

Releases the ingress claim and schedules a retry with exponential back-off.
After 5 failed attempts, state transitions to `failed` instead.

---

### `fail_mail_ingress`

```rust
pub fn fail_mail_ingress(
    ctx: &ReducerContext,
    ingress_id: String,
    error: String,
) -> Result<(), String>
```

Permanently marks the ingress as `failed`. Use when a non-transient error is detected.

---

### `enqueue_mail_delivery`

```rust
pub fn enqueue_mail_delivery(
    ctx: &ReducerContext,
    ingress_id: String,
    subscription_id: u64,
    recipient_email: String,
    recipient_account_id: Option<u64>,
    list_email: String,
    list_name: String,
    original_sender_email: String,
    from_header: String,
    reply_to: String,
    subject: String,
    body_raw: String,
    headers_raw: String,
    raw_message: String,
    unsubscribe_token: String,
) -> Result<(), String>
```

Creates a `MailDelivery` row (and an initial `queued` `MailDeliveryEvent`) for one subscriber.
Called by the sender daemon during fan-out after it claims a `MailIngress`.

---

### `claim_next_mail_delivery`

```rust
pub fn claim_next_mail_delivery(ctx: &ReducerContext) -> Result<(), String>
```

Atomically claims the next claimable `MailDelivery`. Same eligibility rules as ingress claims
but with a 5-minute lease.

---

### `mark_mail_delivery_sent`

```rust
pub fn mark_mail_delivery_sent(
    ctx: &ReducerContext,
    delivery_id: String,
    smtp_status_code: Option<u16>,
    smtp_response: String,
) -> Result<(), String>
```

Marks a delivery as `sent` and records a `MailDeliveryEvent`. Only the claim owner may call this.

---

### `schedule_mail_delivery_retry`

```rust
pub fn schedule_mail_delivery_retry(
    ctx: &ReducerContext,
    delivery_id: String,
    smtp_status_code: Option<u16>,
    smtp_response: String,
    error_kind: String,
) -> Result<(), String>
```

Releases the delivery claim and schedules a retry with exponential back-off.
After 5 attempts, transitions to `failed`.

---

### `fail_mail_delivery`

```rust
pub fn fail_mail_delivery(
    ctx: &ReducerContext,
    delivery_id: String,
    smtp_status_code: Option<u16>,
    smtp_response: String,
    error_kind: String,
) -> Result<(), String>
```

Permanently fails a delivery.

---

### `mark_mail_delivery_bounced`

```rust
pub fn mark_mail_delivery_bounced(
    ctx: &ReducerContext,
    delivery_id: String,
    smtp_response: String,
    error_kind: String,
) -> Result<(), String>
```

Records a permanent SMTP bounce (5xx) against the delivery and records a `MailDeliveryEvent`
with `smtp_status_code = 550`.

---

## Authorization

Two helper functions are used throughout the module:

```rust
pub(crate) fn is_admin_user(ctx: &ReducerContext) -> bool
pub(crate) fn is_admin_identity(ctx: &ReducerContext, who: Identity) -> bool
```

- The module's own database identity is always treated as admin (allows the HTTP handler to
  authenticate via the module's identity in `ctx.with_tx`).
- Any identity listed in `admin_identities` is also treated as admin.
