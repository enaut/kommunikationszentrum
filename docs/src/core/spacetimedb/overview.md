# SpacetimeDB Server

SpacetimeDB is the core storage and business-logic component. The Rust module contains the database schema, reducers, procedures, scheduled jobs, and HTTP handlers for account synchronization, topic management, inbound mail processing, and outbound delivery.

### Integrations

1. **Admin Web UI** connects through the SpacetimeDB client to read scoped views and call reducers.
2. **Django backend** synchronizes user, topic, category, and subscription data.
3. **Stalwart MTA** sends inbound mail hook requests and provides domain, mailbox, and JMAP services.

### WebSocket subscriptions

The Admin UI subscribes to caller-filtered views. Members receive their own account and subscription data plus messages visible to their subscriptions. Administrators receive management views and JMAP delivery and event views used by the status page.

Outbound work is scheduled inside the module; no external worker subscribes to queue views. Legacy SMTP queue tables remain private to the module while pending rows are handled by the migration procedure. See [Outbound Delivery](../outbound-delivery.md) for the current queue and rollout details.
