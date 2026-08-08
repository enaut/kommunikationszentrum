# MTA Hook Processing

The module handles Stalwart MTA webhook requests for each SMTP stage. Handlers receive a JSON body shaped as `stalwart_mta_hook_types::Request` and return a `stalwart_mta_hook_types::Response` describing the action (`accept`, `reject`, or `quarantine`) and optional message modifications.

---

## Stage Processing

| Stage | Purpose | Implementation |
|---|---|---|
| `connect` | Accept or reject the TCP/SMTP connection based on client IP. | Checks `blocked_ips` table; returns `reject` if an active block is present, otherwise logs and returns `accept`. |
| `ehlo` | Validate the HELO/EHLO argument. | Rejects if the argument is empty; otherwise logs and returns `accept`. |
| `mail` | Validate the envelope sender address. | Rejects with 550 if the sender is missing `@` or is empty. Valid senders are logged and accepted. |
| `rcpt` | Verify recipient addresses against known, active message categories. | Performs an indexed lookup on `message_categories.email_address`. Accepts on a match; returns `reject` (550) if no active category matches any recipient. |
| `data` | Process and persist the incoming message for delivery to subscribers. | See [Data Stage Detail](#data-stage-detail) below. |
| `auth` | Preliminary SMTP authentication handling. | Accepts the authentication attempt and logs it (pass-through for this project). |

---

## Data Stage Detail

The `data` handler runs inside `ctx.with_tx(...)` to ensure atomic writes:

1. Extracts headers, subject, message size, and body.
2. Resolves matching categories from envelope recipients; falls back to the message `To` header.
3. For each matching active category, checks whether the sender is subscribed via the `subscriptions` table.
4. If deliveries are found, stores a `received_message` row per delivery and returns `accept` (optionally adding processing headers). If no deliveries are possible, quarantines the message.

> **Note:** Messages over 2 MB may have their bodies omitted from storage to avoid memory pressure.

---

## Logging & Auditing

| Table | Content |
|---|---|
| `mta_connection_log` | Per-connection events (CONNECT, EHLO, MAIL, RCPT, AUTH stages). |
| `mta_message_log` | Per-message events (DATA stage). Sensitive fields such as client IPs are redacted in public logs. |

Use the `dump_mta_logs_to_server_logs` reducer to print MTA logs to the module's console output for debugging.
