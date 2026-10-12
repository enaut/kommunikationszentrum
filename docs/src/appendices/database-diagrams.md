# Database Diagrams

This appendix contains database and processing diagrams for the Kommunikationszentrum SpacetimeDB schema.

## Complete Database Schema

```d2
{{#include database-schema-complete.d2}}
```

The complete database schema diagram shows:

- **All tables** with their field names and data types
- **Primary keys** (PK) and auto-increment fields
- **Foreign key relationships** with solid blue arrows
- **Logical relationships** with dashed blue arrows
- **Functional grouping** of tables by color:
  - **Yellow (User Management)**: `account`, `account_emails`, `email_verification_tokens`, `account_configs`
  - **Green (Topic & Category Management)**: `message_topics`, `categories`, `message_topic_categories`, `subscriptions`
  - **Blue (MTA & Retention)**: `blocked_ips`, `received_message`, `mail_retention_config`, `purge_rejected_mails_schedule`
  - **Purple (Delivery & System Mail)**: `mail_message`, `mail_ingress`, `system_mail_pending`

## Simplified Entity-Relationship Diagram

```d2
{{#include database-schema-er-simplified.d2}}
```

## MTA Processing Flow

```d2
{{#include mta-processing-flow.d2}}
```

The MTA processing flow diagram illustrates:

### Processing Stages

1. **CONNECT**: Initial connection validation
   - IP blocking check against `blocked_ips` table
   - Decision: ACCEPT or REJECT (550). Zero database writes.

2. **EHLO / HELO**: Protocol compliance
   - Pass-through ACCEPT. Zero database writes.

3. **MAIL FROM**: Sender envelope reception
   - Pass-through ACCEPT (permits `<>` bounce messages). Zero database writes.

4. **RCPT TO**: Recipient envelope reception
   - Pass-through ACCEPT (ensures personal mail to `bob@solawis.de` passes uninterrupted). Zero database writes.

5. **DATA**: Envelope topic detection and admission policy
   - Non-topic mail: Immediate pure pass-through ACCEPT (no header modifications, no DB writes).
   - Topic mail:
     - Oversize pre-check (> 2 MB): Dispatched with disposition `Rejected(Oversize)` and empty body (`body_raw: ""`) in SpacetimeDB.
     - Admission evaluation: Admin identity bypass, or verified email + active account + active `Write` subscription.
     - Rejection reasons: `NotRegistered`, `NotSubscribed`, `NoWritePermission`, `Oversize`.
     - Stalwart response: Appends ` (rejected from <topic1>, ...)` to Subject header if any topic was rejected.
     - Clean original subject preserved in SpacetimeDB for subscriber fan-out to accepted topics.
     - Accepted topics enqueue ingress fan-out jobs in `mail_ingress`.

6. **AUTH**: Authentication
   - Pass-through ACCEPT. Zero database writes.

### Decision Points

- **ACCEPT**: Pass through to Stalwart or queue for subscriber fan-out.
- **REJECT**: Immediate 550 for blocked IPs at CONNECT; or per-topic rejection recorded in `received_message` at DATA stage with subject suffixing in Stalwart.

### Database Interactions

- **Lookups**: `blocked_ips`, `message_topics`, `account`, `account_emails`, `admin_identities`, `subscriptions`
- **Mutations (DATA stage only)**: `mail_message`, `received_message`, `mail_ingress` (accepted topics only), `system_mail_pending` (rejection notices)
- **Retention Purge**: Scheduled reducer `purge_rejected_mails` purges rejected `received_message` and unreferenced `mail_message` rows after the configured retention period (`mail_retention_config.rejected_retention_days`).

---

## Diagram Rendering with mdbook-d2

All diagrams in this documentation are rendered using mdbook-d2, compiling D2 syntax directly during mdBook building:

- **Vector output**: SVGs generated at build time that scale crisply and support dark mode.
- **Plain-text diagram definitions**: Fully diff-friendly and version-controlled alongside the markdown documentation.

### Updating Diagrams

To update a diagram, edit the corresponding `.d2` file in `docs/src/` and rebuild:

```bash
cd docs
mdbook build
# or for live preview:
mdbook serve
```
