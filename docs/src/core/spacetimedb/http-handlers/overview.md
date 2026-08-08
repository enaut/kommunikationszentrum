# Webhooks & Module HTTP Handlers

The Kommunikationszentrum SpacetimeDB module exposes a small set of HTTP handlers that receive external webhooks (MTA hooks and user synchronisation requests), authenticate the caller, and perform the corresponding database operations inside transactional contexts.

HTTP handlers are implemented using the `#[spacetimedb::http::handler]` macro and registered with a `#[spacetimedb::http::router]` function. All module routes are mounted under the SpacetimeDB host path `/v1/database/:name_or_identity/route/{*path}`.

---

## Request Flow

```d2
{{#include http-handler-flow.d2}}
```

---

## Route Overview

| Route | Method | Permission | Description |
|---|---|---|---|
| `/v1/database/kommunikation/route/mta-hook` | POST | `mta-hook` | Receives Stalwart MTA webhook events for each SMTP stage. |
| `/v1/database/kommunikation/route/user-sync` | POST | `sync-user` | Receives user account synchronisation requests from Django. |

---

## Security Model

The host delegates `Authorization` handling entirely to module handlers. As a result, `HandlerContext` does not expose a `sender()` identity — handlers must verify the bearer token themselves before executing any logic.

| Concept | Detail |
|---|---|
| Token storage | Only a BLAKE3 cryptographic hash of the plaintext token is stored in the `webhook_tokens` table. |
| Token permissions | Each token carries a `permissions: Vec<String>` column; the handler checks the required permission for the route before proceeding. |
| Token management | Create and revoke tokens via admin-only reducers (`generate_webhook_token`, `revoke_webhook_token`). The Admin Web UI (Debug → Webhook Tokens) can generate and hash tokens client-side without CLI access. |

---

## Implementation Notes

- HTTP handler source lives in `server/src/http_handlers.rs`.
- MTA stage-processing helpers are implemented in `server/src/mta.rs`. Handlers invoke the same helpers inside `ctx.with_tx(...)`, so behaviour is identical whether triggered via an HTTP webhook or an internal reducer call.
- Database writes inside handlers must use `ctx.with_tx(|tx| { ... })` to ensure atomicity. Handlers must not hold transactions across blocking I/O.
