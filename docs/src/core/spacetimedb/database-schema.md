# Database Schema

The SpacetimeDB module for Kommunikationszentrum is the canonical state for user identity, category subscriptions, domain metadata, MTA audit records, and the outbound delivery pipeline. The schema is intentionally private to the module; client access is restricted to scoped public views.

## Full Entity-Relationship Schema

The module stores identity, category, and delivery state in private tables, while admin/member visibility is enforced by public views such as `visible_accounts`, `visible_message_categories`, and `visible_domains`.

```d2
{{#include database-schema-er.d2}}
```

---

## Delivery Pipeline Architecture

The delivery pipeline separates inbound message ingestion from outbound SMTP work. A `mail_ingress` row is created once per category and message, then one `mail_delivery_pending` row is generated per recipient subscription. Transient SMTP failures move the delivery row into `mail_delivery_temporary_failed`, where a scheduled reducer requeues rows after their `next_attempt_at` deadline.

```d2
{{#include database-schema-delivery-pipeline.d2}}
```

---

## Subscription Lifecycle & Unsubscribe Flow

Subscriptions carry the current member state across automatic Django syncs and explicit user or admin actions. The status model tracks automatic subscriptions, required assignments, manual actions, and one-click unsubscribe actions.

```d2
{{#include database-schema-subscription-lifecycle.d2}}
```

> **Sync protection rule:** Django owns `RequiredSubscribed` assignments and may add or remove them as membership data changes. The sync path will not overwrite `ManuallySubscribed`, `ManuallyUnsubscribed`, or `LinkUnsubscribed` states, and a member cannot remove a required subscription through a list-unsubscribe link.

---

## Client Visibility Views

SpacetimeDB tables remain module-private. Public views enforce caller-specific visibility for members, admins, and the sender worker.

```d2
{{#include database-schema-client-views.d2}}
```
