# Control Flow

This page details the runtime control flow of the `sender` daemon, including startup, the end-to-end delivery lifecycle, reactive event looping, fan-out processing, SMTP dispatch, and automated crash recovery.

---

## 1. End-to-End Delivery Lifecycle

The entire sequence from MTA hook ingestion through SpacetimeDB table transitions to SMTP submission:

```d2
{{#include control-flow-lifecycle-sequence.d2}}
```

---

## 2. Startup Sequence

The daemon initializes tracing, connects to SpacetimeDB, subscribes to views, spawns the background connection pump, registers table callbacks, and triggers a bootstrap doorbell.

```d2
{{#include control-flow-startup-sequence.d2}}
```

---

## 3. Main Reactive Work Loop

The event loop multiplexes shutdown signals and reactive database update notifications using `tokio::select!`.

```d2
{{#include control-flow-main-event-loop.d2}}
```

> **Chain Reaction:** When `process_fanout_jobs` or `process_delivery_jobs` completes a unit of work, `notify.notify_one()` is immediately triggered again to drain backlogs without sleeping.

---

## 4. Ingress Fan-Out Flow (`process_fanout_jobs`)

Fan-out reads inbound `MailIngress` jobs and generates individual `MailDeliveryPending` rows for all active subscribers:

```d2
{{#include control-flow-fanout-jobs.d2}}
```

### Ingress Job Processing (`process_ingress_job`)

```d2
{{#include control-flow-ingress-job.d2}}
```

### Subscriber Job Processing (`process_subscription_job`)

```d2
{{#include control-flow-subscription-job.d2}}
```

---

## 5. Delivery Dispatch Pipeline (`process_delivery_jobs`)

Queued deliveries are claimed atomically, converted to RFC 5321 envelopes, and dispatched via SMTP:

```d2
{{#include control-flow-delivery-jobs.d2}}
```

### SMTP Execution & Status Classification (`send_delivery`)

```d2
{{#include control-flow-send-delivery.d2}}
```

### Backoff & Retry Strategy

When SMTP returns transient errors (4xx or network timeouts), deliveries are requeued with exponential backoff:

| Attempt | Backoff Delay |
|---|---|
| 1 | 30 seconds |
| 2 | 2 minutes |
| 3 | 10 minutes |
| 4 | 30 minutes |
| 5 | 60 minutes |
| > 5 | 12 hours (marked `failed` after max attempts) |

---

## 6. Lease Expiration & Crash Recovery

If a daemon crashes while holding an active ingress or delivery lease, SpacetimeDB's scheduled cleanup recycler automatically recovers the work:

```d2
{{#include control-flow-lease-expiration.d2}}
```

- **Ingress Lease Duration:** 10 minutes (`claim_expires_at`).
- **Delivery Lease Duration:** 5 minutes (`lease_expires_at`).
- **Recycle Cron:** Every 60 seconds via `expire_stale_delivery_claims`.

---

## 7. Outbound Message Headers

`compose_delivery` in `mail.rs` constructs RFC 5322 messages with standardized mailing list headers:

| Header | Value | Description |
|---|---|---|
| `From` | `category.name <category.email_address>` | List address |
| `To` | `subscription.subscriber_email` | Individual recipient address |
| `Reply-To` | `original_sender_email` | Direct replies to original author |
| `Subject` | `[ListName] <original_subject>` | List prefix ensured |
| `Message-ID` | `<seed@domain>` | Unique generated ID |
| `List-Id` | `ListName <category.email_address>` | RFC 2919 List Identifier |
| `List-Post` | `<mailto:category.email_address>` | Posting address |
| `List-Unsubscribe` | `<mailto:...>, <https://.../unsubscribe?token=...>` | One-click & HTTPS unsubscribe |
| `List-Unsubscribe-Post` | `List-Unsubscribe=One-Click` | RFC 8058 one-click support |
| `Precedence` | `list` | Legacy mailing list marker |
| `X-BeenThere` | `category.email_address` | Loop prevention header |
