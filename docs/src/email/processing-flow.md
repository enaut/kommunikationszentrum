# Processing Flow

The email processing flow in the Kommunikationszentrum follows a multi-stage validation process based on MTA hooks from Stalwart.

## MTA Processing Pipeline

```d2
{{#include processing-pipeline.d2}}
```

## Processing Stages

### 1. CONNECT Stage
- **Purpose**: Initial TCP/SMTP connection check.
- **Checks**: Evaluates the client IP against the `blocked_ips` table.
- **Actions**:
  - `reject(550, "IP blocked")`: Returned immediately if the IP exists and is active.
  - `accept()`: Returned if the IP is not blocked.
- **Zero Database Writes**: Performed in memory/read-only transaction without connection logging, eliminating write amplification.

### 2. EHLO/HELO Stage
- **Purpose**: Protocol compliance.
- **Actions**: Pure pass-through (`accept()`). Zero database writes or lookups.

### 3. MAIL FROM Stage
- **Purpose**: Envelope sender reception.
- **Actions**: Pure pass-through (`accept()`). Zero database writes or lookups.
- **Bounce Compatibility**: Permitting all senders without pre-DATA rejection ensures automated bounce messages with null reverse-path (`MAIL FROM: <>`) reach intended destinations.

### 4. RCPT TO Stage
- **Purpose**: Recipient envelope reception.
- **Actions**: Pure pass-through (`accept()`). Zero database writes or lookups.
- **Personal Mail Delivery**: Recipient addresses are not rejected at the RCPT stage. This ensures regular personal mail (e.g., `bob@solawis.de`) and multi-recipient messages pass through without interference. Topic validation is performed strictly in the DATA stage.

### 5. DATA Stage
- **Purpose**: Envelope-only topic detection, sender admission evaluation, message persistence, subject tagging, and fan-out dispatch.
- **Step 1: Envelope-Only Topic Detection**:
  - The handler inspects only SMTP envelope recipients (`request.envelope.to`), ignoring message `To:`/`Cc:` headers.
  - Addresses are checked against active topics in `message_topics`.
- **Step 2: Non-Topic Pass-Through**:
  - If no envelope recipient matches an active topic, the handler immediately returns `accept()` with zero modifications, no `X-Processed-By` header, and zero database writes. Stalwart delivers personal mail normally.
- **Step 3: Admission Policy Evaluation**:
  - Oversize check: Evaluated before authorization. If `message_size > 2,000,000` bytes (2 MB), the disposition is `Rejected(Oversize)`.
  - Sender resolution: Checks the `From` header address, falling back to envelope sender. Accounts associated with this address in `account_emails` must be active (`is_active == true`) and the email verified (`is_verified == true`).
  - **Admin Bypass**: If any matching account holds an admin identity in `admin_identities`, posting authorization is granted (`Accepted`).
  - **Member Admission**: Otherwise, the sender must have an active subscription with `SubscriptionPermission::Write` for that exact email address on the topic (`Accepted`).
  - If admission fails, one of the 4 rejection reasons is assigned:
    1. `TopicRejectionReason::NotRegistered`: Sender email is unknown, unverified, or associated with an inactive account.
    2. `TopicRejectionReason::NotSubscribed`: Sender has an active account, but no active subscription to this topic on that address.
    3. `TopicRejectionReason::NoWritePermission`: Sender has an active subscription, but with `SubscriptionPermission::Read`.
    4. `TopicRejectionReason::Oversize`: Message size exceeds 2 MB.
- **Step 4: Persistence & Storage Optimization**:
  - SpacetimeDB stores a canonical `mail_message` row.
  - **RAM Optimization for Oversize Mails**: If oversize, `body_raw` is stored as an empty string (`""`) in SpacetimeDB to protect RAM, while Stalwart retains the full message body on disk/S3. The Admin UI displays an indicator that the body is only available in Stalwart.
  - A `received_message` row is recorded for each targeted topic with its `disposition` (`Accepted` or `Rejected(reason)`).
  - For accepted topics, a `mail_ingress` record is inserted to trigger background sender fan-out.
  - For rejected topics, no `mail_ingress` record is created, preventing distribution to subscribers. An explanatory rejection notice from `SMTP_SYSTEM_USER` is queued in `system_mail_pending` (unless the sender is a bounce address `<>`).
- **Step 5: Stalwart Hook Response & Subject Modification**:
  - Adds header `X-Processed-By: SpacetimeDB Kommunikationszentrum`.
  - If any topic was rejected, appends ` (rejected from <topic1>, <topic2>)` to the `Subject` header via Stalwart `Modification::change_header(1, "Subject", ...)`.
  - SpacetimeDB preserves the original clean subject in `mail_message.subject`, so subscriber fan-out to accepted topics carries the clean subject.
  - All envelope recipients remain intact (no `DeleteRecipient` modifications).

### 6. AUTH Stage
- **Purpose**: Inbound SMTP authentication check.
- **Actions**: Pure pass-through (`accept()`). Zero database writes.

---

## Retention & Admin Visibility

- **Visibility**: Non-admin members never see rejected message rows in their views. Admin accounts can view all accepted and rejected messages in the Admin Messages tab.
- **Purge**: A scheduled reducer (`purge_rejected_mails`) runs periodically to remove rejected `received_message` rows older than a configurable retention cutoff (configured via `mail_retention_config.rejected_retention_days`, default 30 days). Unreferenced `mail_message` records are cleaned up automatically.