# Webhooks (Module HTTP Handlers) — Setup

This page describes how to configure external systems (Stalwart MTA, Django) to call the module HTTP routes exposed by the Kommunikationszentrum SpacetimeDB module.

1) Publish the module

- Build and publish the module using the `spacetime` CLI from the repository root:

  ```bash
  spacetime build -p server
  spacetime publish --project-path server kommunikation
  ```

2) Create a webhook token

- Choose a secure, long random token value and create it in the module. Only admin identities may create tokens:

  ```bash
  spacetimedb_call="spacetime call kommunikation"
  $spacetimedb_call create_webhook_token "s3cure-token-value" "django-sync" '["sync-user"]'
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
