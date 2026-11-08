# Database Schema

The SpacetimeDB module is the canonical store for identity, topic subscriptions, categories, domain metadata, MTA records, Stalwart configuration, and mail delivery state. Client access is restricted to public views.

### Entity relationship schema

```d2
{{#include database-schema-er.d2}}
```

### Delivery state

Accepted inbound messages are stored as canonical `mail_message` records with `mail_ingress` rows. The scheduled JMAP pipeline expands ingress into durable per-recipient `jmap_mail_deliveries` and records lifecycle events in `jmap_mail_delivery_events`. The dispatcher uploads, imports, and submits each recipient message through Stalwart JMAP; state is retained for retries and administrator status reporting.

The legacy SMTP queue tables remain in the schema for migration and reconciliation. `migrate_pending_legacy_deliveries` migrates unclaimed pending and temporary-failure rows. Claimed legacy rows require outcome reconciliation before they can safely be retried. See [Outbound Delivery](../outbound-delivery.md).

```d2
{{#include database-schema-delivery-pipeline.d2}}
```

### Subscription lifecycle and visibility

Subscriptions track synchronized assignments, member choices, and one-click unsubscribe actions.

```d2
{{#include database-schema-subscription-lifecycle.d2}}
```

Django owns `RequiredSubscribed` assignments. Synchronization does not overwrite manual or link-unsubscribe states, and a member cannot remove a required subscription through a list-unsubscribe link.

Tables remain private. Public views expose caller-specific member and administrator data; only administrators can read JMAP delivery and event status. See the [client visibility diagram](./database-schema-client-views.d2).
