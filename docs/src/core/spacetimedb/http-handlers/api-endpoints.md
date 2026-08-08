# API Endpoints Reference

All module HTTP routes are mounted under the SpacetimeDB host path:

```
/v1/database/:name_or_identity/route/{*path}
```

---

## Endpoints

| Method | Path | Permission | Content-Type | Description |
|---|---|---|---|---|
| POST | `/v1/database/kommunikation/route/mta-hook` | `mta-hook` | `application/json` | Receives Stalwart MTA hook events. Request: `stalwart_mta_hook_types::Request`. Response: `stalwart_mta_hook_types::Response`. |
| POST | `/v1/database/kommunikation/route/user-sync` | `sync-user` | `application/json` | Receives user account sync requests from Django. See [User Synchronisation](./user-sync.md) for the full payload shape. |

---

## Authentication

All requests must include a bearer token in the `Authorization` header:

```
Authorization: Bearer <token>
```

Tokens are created via the admin-only `generate_webhook_token` reducer and stored as a BLAKE3 hash in the `webhook_tokens` table. Each token carries a `permissions` list; the handler rejects requests whose token lacks the required permission for the route.

---

## Request Examples

**MTA Hook (RCPT stage):**

```bash
curl -X POST "http://localhost:3000/v1/database/kommunikation/route/mta-hook" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '@test_data/rcpt_hook.json'
```

**User sync:**

```bash
curl -X POST "http://localhost:3000/v1/database/kommunikation/route/user-sync" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{"action":"upsert","user":{"mitgliedsnr":12345,"name":"Test","email":"test@example.org","is_active":true}}'
```
