# MTA Hook Processing

The module handles Stalwart MTA webhook requests for each SMTP stage. Handlers receive a JSON body shaped as `stalwart_mta_hook_types::Request` and return a `stalwart_mta_hook_types::Response` describing the action (`accept` or `reject`) and optional message modifications.

---

## Authentication

All calls to `POST /v1/database/kommunikation/route/mta-hook` require an `Authorization: Bearer <token>` header with a webhook token that has the `mta-hook` permission.

For instructions on generating a token, hashing it with BLAKE3, and registering it with the module, see [Managing Webhook Tokens / Token Generation](../module-publishing.md#managing-webhook-tokens).

---

## Stage Processing

| Stage | Purpose | Implementation |
|---|---|---|
| `connect` | Check client IP against blocklist. | Checks `blocked_ips` table; returns `reject(550, "IP blocked")` if an active block is present, otherwise returns `accept()`. Zero database writes or log entries. |
| `ehlo` | Protocol compliance. | Pure pass-through: returns `accept()`. Zero database writes. |
| `mail` | Envelope sender handling. | Pure pass-through: returns `accept()`. Allows bounce senders with `<>` reverse-path. Zero database writes. |
| `rcpt` | Recipient envelope handling. | Pure pass-through: returns `accept()`. Recipient checks are intentionally deferred to the `data` stage so non-topic personal mail (e.g., `bob@solawis.de`) passes uninterrupted. Zero database writes. |
| `data` | Envelope-only topic matching, admission policy evaluation, message persistence, subject modification, and delivery queueing. | See [Data Stage Detail](#data-stage-detail) below. |
| `auth` | SMTP authentication. | Pure pass-through: returns `accept()`. Zero database writes. |

---

## Data Stage Detail

The `data` handler runs inside `ctx.with_tx(...)` to ensure atomic state updates:

1. **Strict Envelope Matching**:
   Inspects only SMTP envelope recipients (`request.envelope.to`) to identify targeted active topics from `message_topics`. Message headers (`To:`, `Cc:`) are never used for topic routing.

2. **Non-Topic Pass-Through**:
   If no envelope recipient matches an active topic, the handler returns immediately with `MtaHookResponse::accept()`, containing zero modifications, no `X-Processed-By` header, and zero database writes. Stalwart delivers personal mail without modification.

3. **Admission Decision Logic**:
   - **Oversize Pre-check**: If `message_size > 2,000,000` bytes (2 MB), the message is assigned disposition `Rejected(TopicRejectionReason::Oversize)`.
   - **Sender Account Resolution**: Sender address is identified by checking the `From` header address, falling back to envelope sender. Accounts in `account_emails` must be active (`is_active == true`) and verified (`is_verified == true`).
   - **Admin Bypass**: If any matching account has an admin identity in `admin_identities`, posting is authorized (`Accepted`).
   - **Subscriber Permission**: For non-admins, the sender's exact address must hold an active subscription to the topic with `SubscriptionPermission::Write`.
   - **Rejection Reasons**:
     - `NotRegistered`: Sender address is unknown, unverified, or belongs to an inactive account.
     - `NotSubscribed`: Sender has an active account, but no active subscription to this topic on that address.
     - `NoWritePermission`: Sender has an active subscription, but with `SubscriptionPermission::Read`.
     - `Oversize`: Message size exceeds 2 MB limit.

4. **Persistence & RAM Optimization**:
   - Inserts a canonical `MailMessage` row. For oversized messages, `body_raw` is stored as an empty string (`""`) to conserve SpacetimeDB RAM, while Stalwart retains the complete payload on disk/S3.
   - Records a `ReceivedMessage` entry for each target topic with its admission `disposition` (`Accepted` or `Rejected(reason)`).
   - If accepted: Enqueues a `MailIngress` fan-out record for background processing by the sender daemon.
   - If rejected: Queues an automated explanatory notice in `SystemMailPending` from `SMTP_SYSTEM_USER` to the sender (unless the sender is `<>`).

5. **Stalwart Hook Response Modifications**:
   - Adds header `X-Processed-By: SpacetimeDB Kommunikationszentrum`.
   - If any topic was rejected: Appends ` (rejected from <topic1>, <topic2>)` to the `Subject` header via Stalwart `Modification::change_header(1, "Subject", ...)`.
   - Preserves all envelope recipients (no recipients are removed).
   - SpacetimeDB retains the clean original subject in `mail_message.subject` for fanning out to subscribers of accepted topics.

---

## Retention & Purge

- **Admin Visibility**: Non-admin members only see messages delivered to topics they subscribe to. Admin users can view all accepted and rejected messages in the Admin Messages tab.
- **Configurable Purge**: The `purge_rejected_mails` scheduled reducer runs periodically to purge rejected `received_message` rows older than `mail_retention_config.rejected_retention_days` (default 30 days) along with any orphaned `mail_message` rows.
