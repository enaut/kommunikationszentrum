# MTA Hook Processing

The module handles Stalwart MTA webhook requests for each SMTP stage. Handlers receive a JSON body shaped as `stalwart_mta_hook_types::Request` and return a `stalwart_mta_hook_types::Response` describing the action (`accept`, `reject`, or `quarantine`) and optional message modifications.

---

## Authentication

All calls to `POST /v1/database/kommunikation/route/mta-hook` require an `Authorization: Bearer <token>` header with a webhook token that has the `mta-hook` permission.

For instructions on generating a token, hashing it with BLAKE3, and registering it with the module, see [Managing Webhook Tokens / Token Generation](../module-publishing.md#managing-webhook-tokens).

---

## Stage Processing

| Stage | Purpose | Implementation |
|---|---|---|
| `connect` | Accept or reject the TCP/SMTP connection based on client IP. | Checks `blocked_ips` table; returns `reject` if an active block is present, otherwise logs and returns `accept`. |
| `ehlo` | Validate the HELO/EHLO argument. | Rejects if the argument is empty; otherwise logs and returns `accept`. |
| `mail` | Validate the envelope sender address. | Rejects with 550 if the sender is missing `@` or is empty. Valid senders are logged and accepted. |
| `rcpt` | Verify recipient addresses against known, active message topics. | Performs an indexed lookup on `message_topics.email_address`. Accepts on a match; returns `reject` (550) if no active topic matches any recipient. |
| `data` | Process and persist the incoming message for delivery to subscribers. | See [Data Stage Detail](#data-stage-detail) below. |
| `auth` | Preliminary SMTP authentication handling. | Accepts the authentication attempt and logs it (pass-through for this project). |

---

## Data Stage Detail

The `data` handler runs inside `ctx.with_tx(...)` to ensure atomic writes:

1. Extracts headers, subject, message size, and body.
2. Resolves matching topics from envelope recipients; falls back to the message `To` header.
3. If the message is larger than 2,000,000 bytes and addresses an active topic, the DATA handler creates no subscriber archive or fan-out jobs. It records `reject_oversize` and queues a bilingual system mail explaining the size limit for a valid sender address. The DATA hook still accepts the message at the MTA boundary so the queued notice can be delivered.
4. Resolves active sender accounts and checks authorization for each remaining topic:
   - **Admin Access**: If *any* matching account has an admin identity in `admin_identities`, posting authorization is granted.
   - **Member Write Permission**: Otherwise, ensures matching accounts exist and are active (`is_active == true`), and verifies that at least one matching account holds an active subscription with `SubscriptionPermission::Write` to the topic.
5. If authorized, creates a canonical `mail_message` row, enqueues an ingress fan-out job in `mail_ingress`, and archives to `received_message` for subscribed members. If no authorized topics remain, the message is quarantined or rejected according to the authorization result.

---

## Logging & Auditing

| Table | Content |
|---|---|
| `mta_connection_log` | Per-connection events (CONNECT, EHLO, MAIL, RCPT, AUTH stages). |
| `mta_message_log` | Per-message events (DATA stage). Sensitive fields such as client IPs are redacted in public logs. |

Use the `dump_mta_logs_to_server_logs` reducer to print MTA logs to the module's console output for debugging.
