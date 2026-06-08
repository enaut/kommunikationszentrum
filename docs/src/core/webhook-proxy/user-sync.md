# User Synchronization

The user synchronization endpoint allows the Django `solawispielplatz` backend to keep SpacetimeDB account data in sync with the canonical user store.

Endpoint

- POST /v1/database/kommunikation/route/user-sync
  - Content-Type: application/json
  - Authorization: Bearer {token} (requires `sync-user` permission)
  - Request body:

```json
{
  "action": "upsert" | "delete",
  "user": {
    "mitgliedsnr": 12345,
    "name": "Full Name",
    "email": "user@example.org",
    "is_active": true,
    "is_admin": false,
    "updated_at": "2024-01-01T12:00:00Z"
  }
}
```

- Success response: 200 OK with JSON `{ "status": "success", "action": "upsert", "mitgliedsnr": 12345 }`
- Failure: 4xx for client errors (e.g., missing token, malformed JSON), 5xx for server errors

How Django should call the endpoint

- Configure the token in Django settings (for example, `settings_local.py`):

```py
SPACETIME_WEBHOOK_TOKEN = "your-long-secret-token"
SPACETIME_SYNC_URL = "http://localhost:3000/v1/database/kommunikation/route/user-sync"  # optional
```

- The included management command and signal handlers will read `SPACETIME_WEBHOOK_TOKEN` and include it in the `Authorization: Bearer <token>` header when posting.

Token lifecycle and permissions

- Create tokens using the admin-only reducer `create_webhook_token`. You can create tokens from the Admin Web UI (Debug → Webhook Tokens); the UI generates a secure token in the browser, computes the BLAKE3 hex hash client-side, and sends only the hash to the module. Using the UI means the CLI is not required for token creation.

- If you prefer the CLI, compute the BLAKE3 hex hash locally and pass the hash (not the plaintext) to the reducer. Example (pseudo):

```bash
HASH=$(python -c "import blake3; print(blake3.blake3(b'supersecret-token').hexdigest())")
spacetime call kommunikation create_webhook_token "$HASH" "django-sync" '["sync-user"]'
```

- Revoke tokens with `revoke_webhook_token` (needs token hash). Consider adding a small admin reducer to revoke by label in the future.

Retries and reliability

- If a sync request fails due to temporary network or server errors, the sender code queues the payload for retry (see the retry queue implementation in `mitgliederverwaltung/signals.py`).
- Ensure your `SPACETIME_WEBHOOK_TOKEN` is configured in `settings_local.py` before running the one-shot synchronization management command.

Testing

- Use `docs/testscripts/test-user-sync.sh` and export `WEBHOOK_TOKEN` to run the script against your local development SpacetimeDB instance.
