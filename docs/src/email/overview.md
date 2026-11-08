# Email System Integration

The email system connects Stalwart with the SpacetimeDB module. Stalwart delivers inbound SMTP hook events to the module; the module validates topic messages and stores accepted ingress. A scheduled module procedure fans accepted messages out to subscribers and submits individual messages to Stalwart through JMAP.

### Core components

1. **Stalwart MTA** accepts inbound SMTP, sends hook requests to the module, manages domains and topic mailboxes, and provides the JMAP API used for outbound submission.
2. **SpacetimeDB module** evaluates admission, records message and ingress state, composes recipient-specific messages, schedules delivery, and tracks retries and outcomes.
3. **Admin interface** manages topics and subscriptions and exposes delivery status to administrators.

### Chapter contents

- [Stalwart MTA Setup](./stalwart-setup.md): listeners, API keys, MTA hook setup, and telemetry.
- [MTA Hook Configuration](./mta-hook-config.md): webhook contract, error handling, and test scripts.
- [Email Topics & Categories](./topics.md): topic definitions, categories, and address validation.
- [Subscription System](./subscriptions.md): member subscriptions, verification, and state transitions.
- [Processing Flow](./processing-flow.md): SMTP-stage admission and persistence.
- [Trigger Flow](./flow-email-triggers.md): inbound processing and scheduled delivery.
- [Outbound Delivery](../core/outbound-delivery.md): JMAP delivery state, retry handling, and migration notes.
