# Email System Integration

The email subsystem connects the external Stalwart Mail Transfer Agent (MTA) with the SpacetimeDB core database and the background sender daemon to provide a secure, authenticated, mailing list distribution platform.

---

## Core Components

```d2
{{#include stalwart-architecture.d2}}
```

1. **Stalwart MTA**:
   - Accepts inbound SMTP sessions on port 25.
   - Forwards each SMTP session stage (CONNECT, EHLO, AUTH, MAIL FROM, RCPT TO, DATA) via HTTP webhooks to SpacetimeDB.
   - Accepts outbound mail submissions from the sender daemon via SMTP on port 465/587 using per-topic credentials.
   - Manages domain inventory and topic mailboxes via JMAP REST API.

2. **SpacetimeDB Module**:
   - Evaluates client IPs against `blocked_ips` in the CONNECT stage and enforces mailing-list admission rules during the DATA stage.
   - Passes non-topic mail through untouched without modifications or database writes.
   - Seeds accepted topic deliveries into the `mail_ingress` delivery pipeline, records per-topic admission outcomes in `received_message`, and queues rejection notifications for rejected topics.

3. **Sender Daemon**:
   - Claims pending ingress jobs from SpacetimeDB.
   - Expands delivery recipients based on active topic subscriptions.
   - Submits outbound messages back through Stalwart using topic application passwords.

---

## Chapter Contents

- [Stalwart MTA Setup](./stalwart-setup.md): Complete setup guide including listener configuration (port 8093), admin API keys, MTA hook setup, and telemetry with screenshots.
- [MTA Hook Configuration](./mta-hook-config.md): Details on the HTTP webhook contract (`stalwart_mta_hook_types`), error handling, and test scripts.
- [Email Topics & Categories](./topics.md): Topic definitions, categories, schemas, and address validation.
- [Subscription System](./subscriptions.md): Member subscriptions, verification flow, and state transitions.
- [Processing Flow](./processing-flow.md): Step-by-step breakdown of each SMTP stage decision logic.
- [Trigger Flow](./flow-email-triggers.md): Inbound and outbound delivery sequence diagrams.

