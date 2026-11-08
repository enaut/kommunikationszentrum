# Architecture

Kommunikationszentrum stores canonical account, topic, subscription, mail, and delivery state in SpacetimeDB. Django provides identity and user synchronization, Stalwart provides inbound mail handling and JMAP mail services, and the Dioxus application provides member and administrator interfaces.

### System Overview

```d2
{{#include architecture-overview.d2}}
```

### Data Flow

```d2
{{#include architecture-data-flow.d2}}
```

### Current flow

1. Stalwart sends MTA hook requests to the SpacetimeDB module, which evaluates topic admission and records accepted ingress.
2. Django synchronizes account, topic, category, and subscription data.
3. Stalwart domain and mailbox metadata is synchronized with the module.
4. A scheduled procedure in the SpacetimeDB module fans out accepted messages and submits recipient-specific mail through Stalwart JMAP. Durable state supports retries and status reporting.
5. The Dioxus interface reads scoped views and calls reducers for member and administrator workflows.

### Components

#### SpacetimeDB Server (`server`)

- Canonical database for accounts, topics, categories, subscriptions, domains, and delivery state.
- Reducers, procedures, scheduled jobs, and HTTP handlers for user/topic sync, MTA hooks, JMAP delivery, and administration.
- Scoped views for member and administrator access.

#### Admin Web Interface (`admin`)

- Dioxus WebAssembly frontend for member and administrator workflows.
- OAuth login and real-time SpacetimeDB subscriptions.
- Subscription, topic, category, domain, and delivery-status management.

#### External integrations

- **Django solawispielplatz**: identity provider, user synchronization, and OAuth flow.
- **Stalwart MTA**: inbound mail processing, domain inventory, mailbox provisioning, and JMAP mail submission.

For delivery state, migration behavior, and configuration, see [Outbound Delivery](./outbound-delivery.md).
