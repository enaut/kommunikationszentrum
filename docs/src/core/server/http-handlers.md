# HTTP Handlers

The server module exposes three HTTP endpoints through SpacetimeDB's built-in HTTP routing.
All endpoints are available under the module route prefix:

```
POST http://<spacetimedb-host>:<port>/v1/database/<module-name>/route/<path>
```

---

## Authentication

All protected endpoints authenticate callers using **Bearer tokens** from the `Authorization`
header. The provided plaintext token is hashed with BLAKE3 and looked up in `webhook_tokens`.
The token must be `active` and must have the required permission string.

```http
Authorization: Bearer <plaintext-token>
```

> **Security:** The plaintext token is never stored. Only the BLAKE3 hex digest is kept in the
> database. Tokens are created by an admin via the `create_webhook_token` reducer, passing the
> pre-hashed value.

---

## Endpoints

### `POST /mta-hook`

Receives SMTP stage hook requests from the Stalwart MTA and makes synchronous accept/reject
decisions.

**Authentication:** Required. Token must have the `mta-hook` permission.

**Content-Type:** `application/json`

**Request body:** Stalwart `MtaHookRequest` JSON.

**Response:** Stalwart `MtaHookResponse` JSON.

#### Stage Behaviour

| Stage | Decision | Conditions |
|---|---|---|
| `Connect` | Accept | IP not in `blocked_ips` or block is inactive |
| `Connect` | Reject 550 | IP is in `blocked_ips` and `active = true` |
| `Ehlo` | Accept | HELO string is present and non-empty |
| `Ehlo` | Reject 501 | HELO string is empty or missing |
| `Mail` | Accept | `MAIL FROM` contains `@` and is non-empty |
| `Mail` | Reject 550 | `MAIL FROM` is invalid |
| `Rcpt` | Accept | At least one recipient matches an active `message_categories.email_address` |
| `Rcpt` | Reject 550 | No recipient matches any active category |
| `Data` | Accept + `X-Processed-By` header | Message persisted successfully |
| `Auth` | Accept | Always |

#### Data Stage Detail

The DATA stage is the most complex. When accepted:

1. Resolves recipients to `message_categories` via envelope `To` field (with `To`-header
   fallback for MTAs that rewrite envelopes).
2. Looks up the sender's `account` by email.
3. Filters categories: sender must be an admin **or** have an active `Subscription` to that
   category.
4. For each authorized category, inserts a `ReceivedMessage` row and a `MailIngress` row
   (in `pending` state).
5. Returns `MtaHookResponse::accept()` with a `X-Processed-By: SpacetimeDB Kommunikationszentrum`
   header modification.

All persistence runs inside `ctx.with_tx(...)` so the insert is committed before the HTTP
response is sent.

#### Example

```bash
curl -X POST "http://localhost:3000/v1/database/kommunikationszentrum/route/mta-hook" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d @hook_payload.json
```

---

### `POST /user-sync`

Receives user upsert/delete events from Django solawispielplatz and keeps the `account` table
in sync.

**Authentication:** Required. Token must have the `sync-user` permission.

**Content-Type:** `application/json`

**Request body:**

```json
{
  "action": "upsert",
  "user": {
    "mitgliedsnr": 42,
    "name": "Alice Example",
    "email": "alice@example.org",
    "is_active": true,
    "is_admin": false,
    "updated_at": "2026-01-01T00:00:00Z",
    "identity_hex": null
  }
}
```

| Field | Type | Description |
|---|---|---|
| `action` | `"upsert"` \| `"delete"` | Operation to perform |
| `user.mitgliedsnr` | `u64` | Django membership number (used as account `id`) |
| `user.name` | `String?` | Display name |
| `user.email` | `String?` | Primary email |
| `user.is_active` | `bool?` | Account active flag |
| `user.is_admin` | `bool?` | Whether to grant/revoke admin status |
| `user.updated_at` | `String?` | Last modification timestamp from Django |
| `user.identity_hex` | `String?` | Pre-computed SpacetimeDB identity (optional) |

**Responses:**

| Status | Body | Meaning |
|---|---|---|
| 200 | `{"status":"success","action":"…","mitgliedsnr":…}` | Sync applied |
| 400 | `{"error":"…"}` | Invalid JSON |
| 401 | `{"error":"missing Authorization bearer token"}` | No token provided |
| 403 | `{"error":"…"}` | Token lacks `sync-user` permission |
| 500 | `{"error":"…"}` | Internal error |

#### Example

```bash
curl -X POST "http://localhost:3000/v1/database/kommunikationszentrum/route/user-sync" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{"action":"upsert","user":{"mitgliedsnr":42,"name":"Alice","email":"alice@example.org","is_active":true,"is_admin":false}}'
```

---

### `POST /mailing-list/unsubscribe`

RFC 8058 one-click unsubscribe endpoint. Receives a request from a mail client's
`List-Unsubscribe-Post` header action.

**Authentication:** None (token is passed as a query parameter).

**Content-Type:** `application/x-www-form-urlencoded`

**Request:**
```
POST /mailing-list/unsubscribe?token=<unsubscribe-token>
Content-Type: application/x-www-form-urlencoded

List-Unsubscribe=One-Click
```

The `token` query parameter must be a valid `subscription_unsubscribe_tokens.token` value.
The request body must be exactly `List-Unsubscribe=One-Click` as required by RFC 8058.

**Responses:**

| Status | Body | Meaning |
|---|---|---|
| 200 | `{"status":"unsubscribed"}` | Subscription deactivated |
| 400 | `{"error":"missing token query parameter"}` | No token in query string |
| 400 | `{"error":"invalid one-click payload"}` | Body not RFC 8058 compliant |
| 404 | `{"error":"…"}` | Token or subscription not found |
| 405 | — | Non-POST request |
| 500 | `{"error":"…"}` | Internal error |

---

## Router Definition

The router is declared with the `#[spacetimedb::http::router]` macro:

```rust
#[spacetimedb::http::router]
fn router() -> Router {
    Router::new()
        .post("/mta-hook",                 mta_hook_handler)
        .post("/user-sync",                user_sync_handler)
        .post("/mailing-list/unsubscribe", mailing_list_unsubscribe_handler)
}
```

All routes are `POST`-only. Other HTTP methods return `405 Method Not Allowed`.
