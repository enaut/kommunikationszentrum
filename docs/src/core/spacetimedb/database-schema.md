# Database Schema

The SpacetimeDB database for Kommunikationszentrum manages identity and access control, mailing list categories and subscriptions, MTA hook audit logging, and asynchronous mail delivery fan-out.

## Full Entity-Relationship Schema

All underlying tables are module-private. Clients access the data only through the public, access-controlled views described below.

```d2
{{#include database-schema-er.d2}}
```

---

## Delivery Pipeline Architecture

The delivery pipeline decouples MTA message acceptance from subscriber fan-out and outbound SMTP delivery using lease-based worker scheduling.

The delivery row itself no longer carries a `next_attempt_at` timestamp. `MailDeliveryPending` is treated as a FIFO work queue, while transient SMTP failures move rows into a dedicated `mail_delivery_temporary_failed` table that keeps the retry deadline and the failure reason separately. The backend scheduler then re-enqueues expired rows back into pending once the backoff period expires.

```d2
{{#include database-schema-delivery-pipeline.d2}}
```

---

## Subscription Lifecycle & Unsubscribe Flow

Subscriptions track user opt-in status across automated Django syncs and manual user actions.

```d2
{{#include database-schema-subscription-lifecycle.d2}}
```

> **Sync Protection Rule:** Django owns `RequiredSubscribed` subscriptions: it creates them for required assignments and removes them when the assignment ends. A member cannot remove a required subscription through the UI or a `List-Unsubscribe` link; administrators can still remove one explicitly. The sync path does not overwrite `ManuallySubscribed`, `ManuallyUnsubscribed`, or `LinkUnsubscribed` optional subscriptions.

---

## Table Access & Client Visibility Views

SpacetimeDB tables are module-private. Public computed views are the only client-queryable schema surface and enforce caller-specific visibility.

```d2
{{#include database-schema-client-views.d2}}
```
