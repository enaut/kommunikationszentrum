# MTA Hook Configuration

This page documents how the Kommunikationszentrum SpacetimeDB module accepts and processes MTA hooks from Stalwart. The module exposes an HTTP handler that implements per-stage processing and persists message deliveries to the database.

## Hook Protocol

The module implements the standard Stalwart MTA hook protocol. External systems POST a JSON payload for each SMTP stage to the module route. The request format follows the `stalwart_mta_hook_types::Request` type.

### Hook Request Format

A typical hook request contains `context` (stage, client/server metadata), optional `envelope`, and optional `message` fields. See the API Endpoints page for exact examples and test payloads.

### Hook Response Format

Handlers return a `stalwart_mta_hook_types::Response` JSON object with fields similar to:

```json
{
  "action": "accept|reject|quarantine",
  "code": 250,
  "reason": "Message accepted",
  "modifications": [ /* optional server-side headers to add */ ]
}
```

## Stage-specific handling

The module implements stage-specific logic inside the handler. Highlights:

- CONNECT: checks `blocked_ips` and logs connection attempts. Returns `reject` if the IP is actively blocked.
- EHLO/HELO: basic validation of the HELO argument; rejects empty values.
- MAIL FROM: basic sender address validation (syntax checks).
- RCPT TO: fast indexed lookup on `message_categories.email_address` to determine whether to accept the recipient for delivery to a mailing list.
- DATA: extracts headers/body, determines matching categories, checks sender subscriptions, and persists `received_message` rows for each accepted delivery. If no matching deliveries are found, the message is quarantined.
- AUTH: currently a pass-through that logs the authentication attempt.

All persistence is executed inside `ctx.with_tx(...)` transactions to keep operations atomic.

## Deployment and configuration

- The module registers routes under the host path `/v1/database/:name/route/{*path}`. For example:

  - `POST http://localhost:3000/v1/database/kommunikation/route/mta-hook`

- External callers must present `Authorization: Bearer <token>` headers with a token that has the `mta-hook` permission.

- Tokens are created with the `create_webhook_token` reducer and stored only as a BLAKE3 hash in the `webhook_tokens` table.

## Error handling

Errors fall into the following categories:

- Validation errors (invalid envelope, malformed headers) → typically `reject(550, ...)`.
- Temporary server or IO errors → handlers may respond with a temporary failure and the caller should retry. Logged and queued failures can be inspected and retried by operator tooling.
- Unknown conditions → quarantined or rejected depending on severity.

## Testing

- Use `docs/testscripts/test-mta-hooks.sh` to exercise the MTA stages. The script posts to the module route and includes the required bearer token via the `WEBHOOK_TOKEN` environment variable.

- Example curl for the DATA stage:

```bash
curl -X POST "http://localhost:3000/v1/database/kommunikation/route/mta-hook" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d @test_data/data_stage_hook.json
```

## Database integration

- The handler uses indexed lookups and B-Tree filters for efficient category and subscription checks.
- Message and connection logs are written to `mta_message_log` and `mta_connection_log` tables for operational visibility.

## Operational notes

- If running SpacetimeDB in production, place a TLS-terminating reverse proxy in front of the host to protect the HTTP routes.
- Use labeled tokens and rotate them regularly. Revoke tokens you no longer need.
- Monitor the `mta_*` log tables to detect processing anomalies.