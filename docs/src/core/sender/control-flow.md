# Control Flow

This page details the runtime control flow of the `sender` daemon, including startup, the end-to-end delivery lifecycle, reactive event looping, fan-out processing, per-category SMTP dispatch, distributed tracing correlation, and automated crash recovery.

---

## 1. End-to-End Delivery Lifecycle

The entire sequence from MTA hook ingestion through SpacetimeDB table transitions to SMTP submission:

```d2
{{#include control-flow-lifecycle-sequence.d2}}
```

---

## 2. Startup Sequence

The daemon loads configuration, sets up OpenTelemetry exporters, connects to SpacetimeDB, spawns the background connection thread, subscribes to required views, validates admin permissions, and registers table callbacks.

```d2
{{#include control-flow-startup-sequence.d2}}
```

### Table Subscriptions
The daemon subscribes to 10 SpacetimeDB views:
1. `sender_mail_ingress` — Inbound ingress items awaiting fanout.
2. `sender_mail_delivery_pending` — Queued outbound recipient deliveries.
3. `sender_mail_delivery_claimed` — Active delivery leases owned by workers.
4. `sender_mail_delivery_messages` — Full RFC 5322 payload per recipient delivery.
5. `sender_mail_messages` — Canonical message bodies and Stalwart `queue_id`.
6. `active_subscriptions` — Category subscriber lists for fanout.
7. `visible_message_categories` — Active categories and their outbound addresses.
8. `visible_category_app_passwords` — SMTP credentials per category.
9. `active_unsubscribe_tokens` — Per-subscriber one-click unsubscribe tokens.
10. `visible_admin_identities` — List of authorized administrator identities.

### Fail-Fast Admin Identity Verification
When subscriptions are applied (`on_applied` callback), the daemon queries `visible_admin_identities` for its own identity. If missing, it outputs an error with the exact CLI command needed to register the identity and sends `Event::FatalError`, shutting down immediately rather than failing repeatedly during runtime.

---

## 3. Main Reactive Work Loop

The main loop coordinates processing between reactive database callbacks and a 15-second safety timer:

```d2
{{#include control-flow-main-event-loop.d2}}
```

Each loop cycle executes three steps sequentially:
1. `process_fanout_jobs`: Processes owned ingress jobs or requests a new ingress lease.
2. `send_delivery_jobs`: Processes owned delivery jobs over pooled SMTP transports.
3. `claim_next_mail_delivery`: Issues an asynchronous claim for the next pending delivery.

The daemon then suspends on `tokio::select!` awaiting either an incoming `Event::Wakeup` from SpacetimeDB table callbacks or the 15-second fallback poll timer.

---

## 4. Ingress Fan-Out Flow (`process_fanout_jobs`)

Fan-out reads inbound `MailIngress` jobs and generates individual deliveries for all active subscribers:

```d2
{{#include control-flow-fanout-jobs.d2}}
```

### Ingress Job Processing (`process_ingress_job`)

```d2
{{#include control-flow-ingress-job.d2}}
```

#### Safe Token Waiting
If any subscriber does not yet have an active unsubscribe token, `process_ingress_job` requests one via `ensure_subscription_unsubscribe_token` and returns `Err(IngressJobError::AwaitingToken)`. The fanout runner leaves the ingress claimed and retries on the next wakeup **without incrementing the retry counter**, ensuring transient token generation does not burn the ingress attempt limit.

#### Error Isolation
Errors during fanout are strictly categorized:
- **`SubscriptionJobError::Transient`** (e.g. database communication failures) aborts fanout and triggers `retry_mail_ingress`.
- **`SubscriptionJobError::Permanent`** (e.g. malformed recipient email addresses) logs a warning and skips only that specific recipient. Fanout proceeds for all other subscribers.

### Subscriber Job Processing (`process_subscription_job`)

```d2
{{#include control-flow-subscription-job.d2}}
```

- **Idempotency**: If a delivery already exists in `pending`, `claimed`, or `done`, it returns `SubscriptionJobOutcome::AlreadyQueued`, preventing duplicate delivery count increments.
- **Payload Storage**: Message composition builds the full RFC 5322 payload and passes it to `enqueue_mail_delivery()`, which writes the raw message into `sender_mail_delivery_messages`.

---

## 5. Delivery Dispatch Pipeline (`send_delivery_jobs`)

Queued deliveries are claimed atomically, converted to RFC 5321 envelopes, and dispatched via SMTP:

```d2
{{#include control-flow-delivery-jobs.d2}}
```

### Per-Category SMTP Transport Pooling

To prevent SMTP authentication mismatches across different mailing lists, the daemon maintains a local connection pool per category:

```d2
{{#include control-flow-smtp-pooling.d2}}
```

- Transports are cached in a `HashMap<u64, AsyncSmtpTransport<Tokio1Executor>>` keyed by `category_id`.
- Credentials are read securely from `visible_category_app_passwords`.
- If credentials or transport building fails, the delivery is marked permanently failed with `error_kind='smtp-transport-build'`, avoiding endless retries on invalid configuration.

### SMTP Execution & Envelope Addressing (`send_delivery`)

```d2
{{#include control-flow-send-delivery.d2}}
```

- **RFC 5321 Envelope `From`**: Always set to `ingress.category_email` (the mailing list address) so bounces and delivery status notifications return to the list system.
- **Envelope `To`**: Set to `delivery_message.recipient_email`.
- **Pre-SMTP Validation**: If address parsing fails before hitting SMTP, the delivery is permanently failed with `error_kind='pre-smtp'`.

### Delivery Queue Topology & Temporary Failure Backoff

```d2
{{#include control-flow-queue-topology.d2}}
```

When an SMTP relay returns a transient 4xx error or timeout, the sender invokes `schedule_mail_delivery_retry()`. This removes the delivery from `mail_delivery_claimed` and places it into `mail_delivery_temporary_failed` with a 5-minute backoff delay. The backend scheduler (`requeue_temporary_failed_mails_schedule`) runs every 10 seconds to return expired items back into `mail_delivery_pending`.

---

## 6. Distributed Tracing & Tempo Correlation

The sender integrates with OpenTelemetry and correlates with Stalwart MTA transactions:

```d2
{{#include control-flow-tracing.d2}}
```

1. **Queue ID Extraction**: The inbound Stalwart queue ID stored on `MailMessage` is read during `process_ingress_job`.
2. **Deterministic W3C Traceparent**: [`tracing_util::traceparent_from_queue_id`](../../development/sender.md) hashes the queue ID using BLAKE3 into a valid W3C traceparent string (`00-{trace_id}-{span_id}-01`).
3. **Context Re-attachment**: The extracted OTel context is attached to the current span so all sender fanout and delivery spans become children of the Stalwart SMTP transaction in Tempo.
4. **Loki Structured Metadata**: Fields such as `stalwart_trace_id`, `queue_id`, `from`, `to_list`, and `subject` are attached to logs, allowing Grafana dashboards to link directly from log lines to Tempo traces.

---

## 7. Lease Expiration & Crash Recovery

If a daemon crashes while holding an active ingress or delivery lease, SpacetimeDB's scheduled cleanup recycler automatically recovers the work:

```d2
{{#include control-flow-lease-expiration.d2}}
```

- **Ingress Lease Duration:** 10 minutes (`claim_expires_at`).
- **Delivery Lease Duration:** 5 minutes (`lease_expires_at`).
- **Recycle Cron:** Every 60 seconds via `expire_stale_delivery_claims`.

---

## 8. Outbound Message Headers

`compose_delivery` in `mail.rs` uses the `lettre::Message::builder()` API with typed custom headers:

| Header | Value | Description |
|---|---|---|
| `From` | `category.name <category.email_address>` | List display address |
| `To` | `recipient_email` (from AccountEmail) | Individual recipient address |
| `Reply-To` | `original_sender_email` | Direct replies to original author |
| `Subject` | `[ListName] <original_subject>` | Ensured list prefix (reply/forward tags normalized) |
| `Message-ID` | `<seed@domain>` | Unique deterministic ID |
| `List-Id` | `ListName <category.email_address>` | RFC 2919 List Identifier |
| `List-Post` | `<mailto:category.email_address>` | Posting address |
| `List-Unsubscribe` | `<mailto:...>, <https://.../unsubscribe?token=...>` | One-click & HTTPS unsubscribe |
| `List-Unsubscribe-Post` | `List-Unsubscribe=One-Click` | RFC 8058 one-click support |
| `Precedence` | `list` | Legacy mailing list marker |
| `X-Mailing-List` | `ListName` | Header used by mail filters |
| `X-BeenThere` | `category.email_address` | Loop prevention header |
