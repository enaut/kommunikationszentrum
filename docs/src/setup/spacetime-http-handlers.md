# Webhooks (Module HTTP Handlers) — Setup

This page describes how to configure external systems (Stalwart MTA, Django) to call the module HTTP routes exposed by the Kommunikationszentrum SpacetimeDB module.

1) Publish the module

- Build and publish the module using the `spacetime` CLI from the repository root:

  ```bash
  spacetime build -p server
  spacetime publish --project-path server kommunikation
  ```

2) Create a webhook token

- You can create and manage webhook tokens from the Admin Web UI (Debug → Webhook Tokens). The UI generates a secure token in the browser, displays it once for copying, computes the BLAKE3 hex hash client-side, and sends only the hash to the module. Using the UI means you do not need the CLI for token creation.

- If you prefer to use the CLI, compute the BLAKE3 hex hash locally and pass the hash (not the plaintext) to the reducer. For example (pseudo):

  ```bash
  # Compute BLAKE3 hex hash locally (example using python+blake3)
  HASH=$(python -c "import blake3; print(blake3.blake3(b's3cure-token-value').hexdigest())")
  spacetimedb_call="spacetime call kommunikation"
  $spacetimedb_call create_webhook_token "$HASH" "django-sync" '["sync-user"]'
  ```

- Keep the plaintext token secret; the module stores only a BLAKE3 hash of the token.

3) Configure external systems

- Stalwart MTA (MTA hooks): Configure Stalwart to POST hooks to:

  `http://<spacetime-host>:3000/v1/database/kommunikation/route/mta-hook`

  Include the Authorization header:

  `Authorization: Bearer <token>`

- Django (user sync): set the token in `settings_local.py`:

  ```py
  SPACETIME_WEBHOOK_TOKEN = "s3cure-token-value"
  SPACETIME_SYNC_URL = "http://localhost:3000/v1/database/kommunikation/route/user-sync"
  ```

  The included Django management command and signal handlers will read `SPACETIME_WEBHOOK_TOKEN` and send it in the Authorization header.

4) Testing

- Use the repository test scripts (they read the plaintext token from `WEBHOOK_TOKEN` environment variable):

  ```bash
  export WEBHOOK_TOKEN="s3cure-token-value"
  ./docs/testscripts/test-mta-hooks.sh
  ./docs/testscripts/test-user-sync.sh
  ```

5) Operational notes

- Deploy SpacetimeDB with a TLS front-end or reverse proxy if you plan to accept hooks from the public internet.
- Rotate tokens periodically and use labels to track their usage.
- Monitor the `mta_connection_log` and `mta_message_log` tables for operational insight.
