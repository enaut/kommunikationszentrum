# API Endpoints

This page lists the external HTTP endpoints exposed by the Kommunikationszentrum SpacetimeDB module and the expected request/response formats.

All module routes are mounted under the host path:

  /v1/database/:name_or_identity/route/{*path}

For the published module `kommunikation` the current endpoints are:

- POST /v1/database/kommunikation/route/mta-hook
  - Receives MTA hooks from the Stalwart MTA for each SMTP stage.
  - Content-Type: application/json
  - Authorization: Bearer {token} (requires `mta-hook` permission)
  - Request body: `stalwart_mta_hook_types::Request` JSON
  - Response body: `stalwart_mta_hook_types::Response` JSON

- POST /v1/database/kommunikation/route/user-sync
  - Receives user synchronization requests (from Django).
  - Content-Type: application/json
  - Authorization: Bearer {token} (requires `sync-user` permission)
  - Request body: { "action": "upsert" | "delete", "user": UserSyncData }
  - Success response: 200 OK with JSON { status: "success", action: ..., mitgliedsnr: ... }
  - Failure responses: 4xx for client errors, 5xx for server errors

Header and authentication

- Authorization: The module expects a bearer token in the `Authorization` header. Tokens are created with the reducer `create_webhook_token` and stored hashed in the database. Tokens carry a list of permissions; the handler checks the permission required for the endpoint before executing.

Request examples

MTA Hook (RCPT stage) example:

```bash
curl -X POST "http://localhost:3000/v1/database/kommunikation/route/mta-hook" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '@test_data/rcpt_hook.json'
```

User sync example:

```bash
curl -X POST "http://localhost:3000/v1/database/kommunikation/route/user-sync" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{"action":"upsert","user":{"mitgliedsnr":12345,"name":"Test","email":"test@example.org","is_active":true}}'
```

Creating tokens

- Tokens are created via reducer calls that only admins may run. Example using the `spacetime` CLI (run as an admin identity):

```bash
spacetime call kommunikation create_webhook_token "supersecret-token" "label-django-sync" '["sync-user"]'
```

The reducer stores only a hash of the token. Keep the plaintext token secret — it must be stored in calling systems (for example, Django `settings_local.py`) to authenticate requests.

Testing

- Use the test scripts `docs/testscripts/test-mta-hooks.sh` and `docs/testscripts/test-user-sync.sh`. They read the plaintext token from the environment variable `WEBHOOK_TOKEN` and POST to the module routes.
