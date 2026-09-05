# Database Schema

The SpacetimeDB module for Kommunikationszentrum is the canonical state for user identity, category subscriptions, domain metadata, MTA audit records, Stalwart JMAP configuration, and the outbound delivery pipeline. The schema is intentionally private to the module; client access is restricted to scoped public views.

## Full Entity-Relationship Schema

The module stores identity, category, Stalwart configuration, and delivery state in private tables, while admin/member/sender visibility is enforced by scoped public views such as `visible_accounts`, `visible_message_categories`, `visible_domains`, `admin_stalwart_config`, and `sender_mail_delivery_messages`.

```d2
{{#include database-schema-er.d2}}
```

---

## Delivery Pipeline Architecture

The delivery pipeline separates inbound message ingestion from outbound SMTP work and isolates delivery payloads:
1. An inbound email creates a canonical `mail_message` row and a corresponding `mail_ingress` fan-out job.
2. The sender worker claims the ingress job and prepares outbound messages: each recipient receives an immutable `mail_delivery_message` row containing the rendered RFC 5322 payload, and a tracking row in `mail_delivery_pending`.
3. Workers claim pending items (`mail_delivery_claimed`) under short leases.
4. Sent, failed, bounced, or cancelled items terminate in `mail_delivery_done` with a `DeliveryFinalState`. Transient SMTP errors transition to `mail_delivery_temporary_failed`, where the scheduled `requeue_temporary_failed_mails` reducer moves them back to `mail_delivery_pending` once their `next_attempt_at` expires.

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
