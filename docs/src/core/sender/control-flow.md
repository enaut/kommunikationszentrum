# Control Flow

This page describes the complete runtime control flow of the sender daemon, from startup to
shutdown, including the reactive event loop, fan-out algorithm, SMTP dispatch, and error
handling.

---

## 1. Startup Sequence

Startup is a linear sequence of steps that initializes the daemon and connects to SpacetimeDB.

```d2
{{#include control-flow-startup-sequence.d2}}
```

### Key Startup Steps

1. **Config** — `SenderConfig::from_env()` reads all `SMTP_*`, `SPACETIMEDB_*`, and
   `OTLP_*` variables.
2. **Tracing** — OpenTelemetry OTLP exporters (spans + logs) are initialized and bridged into
   `tracing`. The `RUST_LOG` env filter controls log verbosity.
3. **Connection** — `DbConnection::builder()` connects to SpacetimeDB. If `SPACETIMEDB_TOKEN`
   is set, the connection authenticates with the sender's saved identity; otherwise an anonymous
   one is created.
4. **Subscriptions** — Five SQL queries are registered. SpacetimeDB populates the local cache
   with the matching rows and pushes incremental updates as rows change.
5. **DB pump** — `connection.run_async()` drives the SpacetimeDB client's internal I/O loop.
   It runs as a pinned Tokio future alongside the main event loop.
6. **Callbacks** — `on_insert` and `on_update` callbacks on both ingress and delivery tables
   call `notify.notify_one()` to wake the work loop when data changes.
7. **SMTP transport** — `build_transport()` creates a `lettre::SmtpTransport` (reused for all
   deliveries; connection pooling is managed internally by lettre).
8. **Bootstrap notify** — `notify.notify_one()` is called once immediately to process any
   backlogged work that was waiting before the daemon restarted.

---

## 2. Main Event Loop

The daemon runs a `tokio::select!` loop with three branches:

```rust
loop {
    tokio::select! {
        db_res = &mut database_pump => { /* DB pump terminated — fatal error, break */ }
        _ = &mut shutdown_signal   => { /* Ctrl+C — graceful shutdown, break */ }
        _ = notify.notified()      => { /* Work available — process */ }
    }
}
```

graphically:

```d2
{{#include control-flow-main-event-loop.d2}}
```

> **Chain reaction:** If `process_fanout_jobs` or `process_delivery_jobs` reports that it did
> useful work, `notify_one()` is called again immediately. This drains any backlog without
> sleeping — the loop keeps running until there is nothing left to claim.

---

## 3. Fan-Out: `process_fanout_jobs`

Fan-out converts a `MailIngress` (one received email per mailing list) into multiple
`MailDelivery` records (one per subscriber).

```d2
{{#include control-flow-fanout-jobs.d2}}
```

### `process_ingress_job`

For each claimed ingress:

```d2
{{#include control-flow-ingress-job.d2}}
```

### `process_subscription_job`

For each subscriber:

```d2
{{#include control-flow-subscription-job.d2}}
```

---

## 4. Delivery Dispatch: `process_delivery_jobs`

After fan-out creates `MailDelivery` rows in `queued` state, the delivery loop claims and
submits them via SMTP.

```d2
{{#include control-flow-delivery-jobs.d2}}
```

### `send_delivery`

```d2
{{#include control-flow-send-delivery.d2}}
```

**Error classification** (`mail.rs`):
- `is_permanent_error` — `SmtpError::is_permanent()` (5xx)
- `is_transient_error` — `SmtpError::is_transient()` or `is_timeout()` (4xx / network)
- Anything else — treated as transient (schedule retry)

---

## 5. Message Composition: `compose_delivery`

`compose_delivery` in `mail.rs` builds the complete outbound RFC 5322 message for a single
subscriber. It sets the following mailing list headers:

| Header | Value |
|---|---|
| `From` | `category.email_address` (the list address) |
| `To` | `subscription.subscriber_email` |
| `Reply-To` | `ingress.sender_email` (original sender) |
| `Subject` | `[ListName]: <original subject>` (prefix added if not already present) |
| `Message-ID` | `<ingress_id-sub_email@message_id_domain>` |
| `Date` | Current UTC time (RFC 2822) |
| `List-Id` | `ListName <list@domain>` |
| `List-Post` | `<mailto:list@domain>` |
| `List-Unsubscribe` | `<mailto:list?subject=unsubscribe>, <https://…?token=…>` |
| `List-Unsubscribe-Post` | `List-Unsubscribe=One-Click` |
| `Precedence` | `list` |
| `Sender` | `category.email_address` |
| `X-Mailing-List` | `ListName` |
| `X-BeenThere` | `category.email_address` |

The raw SMTP message is `headers (CRLF) + CRLF + ingress.body_raw`. The body is taken
verbatim from the original `MailIngress` — no MIME re-encoding is performed.

---

## 6. In-Flight Tracking & Race Prevention

A key subtlety: after calling `claim_next_mail_ingress()`, the SpacetimeDB subscription update
arrives asynchronously. If the main notify fires again before that update arrives, the local
cache might still show the row as `pending`, causing the claim loop to attempt a double-claim.

The `in_flight_ingresses` and `in_flight_deliveries` `HashSet`s prevent this:

```
1. claim_next_mail_ingress() called
2. ingress.id added to in_flight BEFORE looking at owned jobs
3. Subscription push arrives → on_update callback fires
4. If row left "processing" state → id removed from in_flight
5. Next loop iteration: self_owned_ingress_jobs filters out in_flight IDs
```

---

## 7. Shutdown

Shutdown is triggered by `SIGINT` (Ctrl+C) via `tokio::signal::ctrl_c()`. On receipt:
1. The `select!` loop exits.
2. The OpenTelemetry tracer and logger providers are flushed and shut down.
3. The process exits with `Ok(())`.

In-flight work is not cancelled — any partially processed ingress will have its lease expire
and be re-claimed on the next daemon startup.
