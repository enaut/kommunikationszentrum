# MTA Hook Processing

The SpacetimeDB module implements HTTP handlers that process Stalwart MTA webhook requests for each SMTP stage. Handlers receive a JSON `Request` shaped like the `stalwart_mta_hook_types::Request` type and return a `stalwart_mta_hook_types::Response` object describing the action (accept/reject/quarantine) and optional message modifications.

Request and response formats

Refer to the API Endpoints page for examples. The request contains a `context.stage` field which can be one of: `connect`, `ehlo`, `mail`, `rcpt`, `data`, `auth`.

Stage-specific handling

- CONNECT
  - Purpose: decide whether to accept or reject a TCP/SMTP connection based on client IPs and other metadata.
  - Implementation: the handler checks the `blocked_ips` table for the client IP. If an active block is present the handler returns `reject`; otherwise it logs the connection and returns `accept`.

- EHLO / HELO
  - Purpose: perform basic syntactic validation of the HELO/EHLO argument.
  - Implementation: reject if argument is empty; otherwise return `accept` and log the result.

- MAIL FROM
  - Purpose: validate the envelope sender address for syntactic correctness.
  - Implementation: reject with 550 for invalid sender formats (missing `@` or empty). Valid senders generate an `accept` response and are logged.

- RCPT TO
  - Purpose: verify that the recipient address corresponds to a known, active message category (mailing list) and decide acceptance.
  - Implementation: the handler performs an indexed lookup on `message_categories.email_address` to determine whether the recipient is a known active category. If a match is found the handler accepts immediately. If no active category matches any recipient, the handler returns `reject` (550).

- DATA
  - Purpose: process and persist the incoming message for delivery to category subscribers.
  - Implementation: the DATA handler
    1. Extracts headers, subject, message size and body.
    2. Attempts to determine matching categories from envelope recipients and, as a fallback, from the message `To` header.
    3. For each matching active category, checks whether the sender is subscribed (via the `subscriptions` table).
    4. If matching deliveries are found, the handler stores a `received_message` row per delivery and returns `accept` (optionally adding processing headers). If no deliveries are possible, the handler quarantines the message.
  - Notes: the handler runs the persistence logic inside `ctx.with_tx(...)` to ensure atomic writes. Large messages (over 2 MB) may have their bodies omitted from storage to avoid memory pressure.

- AUTH
  - Purpose: preliminary handling for SMTP authentication stages (pass-through for this project).
  - Implementation: the handler currently accepts the authentication attempt and logs it.

Logging and auditing

- All handlers log events to `mta_connection_log` and `mta_message_log` tables. Sensitive fields such as client IPs are redacted in public logs.
- Use the `dump_mta_logs_to_server_logs` reducer to print MTA logs to the module's console output for debugging.

Best practices

- Keep handlers short and deterministic. Handlers may perform synchronous database work but should avoid long-running external calls.
- Use `ctx.with_tx(|tx| { ... })` when writing to the DB. Do not hold transactions across blocking I/O.
- Ensure external systems provide the required bearer token in `Authorization: Bearer <token>` headers. Tokens are created via `create_webhook_token` reducer and can be revoked with `revoke_webhook_token`. Note: you can create and manage tokens from the Admin Web UI (Debug → Webhook Tokens) — the UI generates the plaintext token in the browser and sends only a BLAKE3 hash to the module, so CLI usage is optional.

Testing

- Run `docs/testscripts/test-mta-hooks.sh` with `WEBHOOK_TOKEN` exported to exercise the MTA stages against your local module.
