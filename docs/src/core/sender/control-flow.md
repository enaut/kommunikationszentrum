# Control Flow

This page describes the complete runtime control flow of the sender daemon, from startup to
shutdown, including the reactive event loop, fan-out algorithm, SMTP dispatch, and error
handling.

---

```d2
direction: down
startup: "Startup" { style.fill: lightyellow }
```

## 1. Startup Sequence

Startup is a linear sequence of steps that initializes the daemon and connects to SpacetimeDB.

```d2
direction: down

env: "Read SenderConfig\nfrom environment" { style.fill: lightyellow }
otel: "Initialize OpenTelemetry\n(OTLP traces + logs → Alloy)" { style.fill: lightyellow }
conn: "Open SpacetimeDB\nWebSocket connection\n(with optional auth token)" { style.fill: lightblue }
sub: "Subscribe to views:\n• sender_mail_ingress\n• sender_mail_deliveries\n• active_subscriptions\n• message_categories\n• active_unsubscribe_tokens" { style.fill: lightblue }
pump: "Start DB async pump\n(connection.run_async())\nas pinned Tokio future" { style.fill: lightblue }
cb: "Register on_insert / on_update\ncallbacks → notify doorbell" { style.fill: lightgreen }
smtp: "Build SmtpTransport\n(TLS/plaintext, credentials)" { style.fill: lightyellow }
boot: "Trigger doorbell once\n(bootstrap any pending work)" { style.fill: lightgreen }
loop_: "Enter tokio::select! loop" { style.fill: lightcoral }

env -> otel -> conn -> sub -> pump -> cb -> smtp -> boot -> loop_
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
direction: down

wait: "tokio::select!\n(waiting for any branch)" {
  shape: diamond
  style.fill: lightyellow
}
db_err: "DB pump terminated\nlog error + break" { style.fill: lightcoral }
ctrl_c: "SIGINT / Ctrl+C\nlog + break" { style.fill: lightcoral }
work: "notify.notified()\nWork loop fires" { style.fill: lightgreen }

fanout: "process_fanout_jobs()\n(ingress → deliveries)" { style.fill: lightblue }
delivery: "process_delivery_jobs()\n(deliveries → SMTP)" { style.fill: lightblue }
more: "Did any work?\nthen notify_one()\n(chain reaction)" {
  shape: diamond
  style.fill: lightgreen
}

wait -> db_err: "db pump done"
wait -> ctrl_c: "signal"
wait -> work: "doorbell rings"
work -> fanout
fanout -> delivery
delivery -> more
more -> wait: "no work left"
more -> work: "yes, loop again"
```

> **Chain reaction:** If `process_fanout_jobs` or `process_delivery_jobs` reports that it did
> useful work, `notify_one()` is called again immediately. This drains any backlog without
> sleeping — the loop keeps running until there is nothing left to claim.

---

## 3. Fan-Out: `process_fanout_jobs`

Fan-out converts a `MailIngress` (one received email per mailing list) into multiple
`MailDelivery` records (one per subscriber).

```d2
direction: down

entry: "process_fanout_jobs()" { style.fill: lightyellow }
owned: "self_owned_ingress_jobs()\nFilter local cache:\nstate=processing,\nclaim_owner=self,\nnot in in_flight set" {
  shape: diamond
  style.fill: white
}
claim: "call claim_next_mail_ingress()\n(reducer → server atomically\nclaims one pending ingress)" { style.fill: lightblue }
wait50: "sleep 50 ms\n(wait for subscription push\nto update local cache)" { style.fill: lightyellow }
recheck: "Re-check owned jobs\n(subscription push arrived?)" {
  shape: diamond
  style.fill: white
}
none: "No new work\nbreak inner loop" { style.fill: lightcoral }
mark: "Add ingress.id to in_flight set\n(prevent double-processing)" { style.fill: lightgreen }
proc: "process_ingress_job()" { style.fill: lightblue }
err: "retry_mail_ingress()\n(error → back-off)" { style.fill: lightyellow }
cont: "continue inner loop\n(look for more work)" { style.fill: lightgreen }

entry -> owned
owned -> claim: "no owned jobs"
owned -> mark: "owned jobs found"
claim -> wait50
wait50 -> recheck
recheck -> none: "still none"
recheck -> mark: "new job appeared"
mark -> proc
proc -> err: "Err(e)"
proc -> cont: "Ok(())"
err -> cont
cont -> owned
```

### `process_ingress_job`

For each claimed ingress:

```d2
direction: down

start: "process_ingress_job(ingress)" { style.fill: lightyellow }
cat: "Lookup message_categories\nby ingress.category_id" { shape: diamond }
fail: "fail_mail_ingress()\n'missing message category'" { style.fill: lightcoral }
subs: "Filter active_subscriptions\nfor this category_id\n+ dedup by email" { style.fill: lightblue }
empty: "No subscribers?\ncomplete_mail_ingress(0, 0)\nreturn Ok" { style.fill: lightgreen }
loop_: "For each subscription:\nprocess_subscription_job()" { style.fill: lightblue }
token: "Any 'AwaitingToken' results?\nreturn Err → retry_mail_ingress()" { style.fill: lightyellow }
done: "complete_mail_ingress(\ningress_id,\ndeliveries_created, 0)" { style.fill: lightgreen }

start -> cat
cat -> fail: "not found"
cat -> subs: "found"
subs -> empty: "empty"
subs -> loop_
loop_ -> token
token -> done: "all queued"
```

### `process_subscription_job`

For each subscriber:

```d2
direction: down

start: "process_subscription_job()" { style.fill: lightyellow }
dup: "Check if MailDelivery already\nexists for this (ingress, subscription)\nID in local cache" { shape: diamond }
skip: "Return DeliveryQueued\n(idempotent skip)" { style.fill: lightgreen }
tok: "Look up active unsubscribe\ntoken from local cache" { shape: diamond }
req: "ensure_subscription_unsubscribe_token()\n→ return AwaitingToken" { style.fill: lightyellow }
comp: "compose_delivery()\nBuild RFC 5322 message\n+ mailing list headers" { style.fill: lightblue }
enq: "enqueue_mail_delivery() reducer\n→ creates MailDelivery row\n(state: queued)" { style.fill: lightblue }
ret: "Return DeliveryQueued" { style.fill: lightgreen }

start -> dup
dup -> skip: "exists"
dup -> tok: "not exists"
tok -> req: "no token"
tok -> comp: "token found"
comp -> enq -> ret
```

---

## 4. Delivery Dispatch: `process_delivery_jobs`

After fan-out creates `MailDelivery` rows in `queued` state, the delivery loop claims and
submits them via SMTP.

```d2
direction: down

entry: "process_delivery_jobs()" { style.fill: lightyellow }
owned: "self_owned_delivery_jobs()\nstate=sending,\nclaim_owner=self,\nnot in in_flight" {
  shape: diamond
  style.fill: white
}
claim: "claim_next_mail_delivery()\n(reducer atomically claims one)" { style.fill: lightblue }
wait50: "sleep 50 ms" { style.fill: lightyellow }
reck: "Re-check owned\ndeliveries" {
  shape: diamond
  style.fill: white
}
none2: "No work\nbreak" { style.fill: lightcoral }
mark2: "Add delivery.id\nto in_flight set" { style.fill: lightgreen }
send: "send_delivery()" { style.fill: lightblue }
cont2: "continue loop" { style.fill: lightgreen }

entry -> owned
owned -> claim: "none owned"
owned -> mark2: "owned found"
claim -> wait50 -> reck
reck -> none2: "still none"
reck -> mark2: "appeared"
mark2 -> send -> cont2 -> owned
```

### `send_delivery`

```d2
direction: right

start: "send_delivery(delivery)" { style.fill: lightyellow }
env: "Build SMTP Envelope\n(from=sender_email,\nto=recipient_email)" { shape: diamond }
pre_err: "fail_mail_delivery()\nerror_kind='pre-smtp'" { style.fill: lightcoral }
smtp: "transport.send_raw(envelope,\nraw_message.as_bytes())" { style.fill: lightblue }
ok: "mark_mail_delivery_sent()\nstatus=200+, smtp_response" { style.fill: lightgreen }
perm: "fail_mail_delivery()\nerror_kind='smtp-permanent'" { style.fill: lightcoral }
trans: "schedule_mail_delivery_retry()\nerror_kind='smtp-transient'\nor 'smtp-unknown'" { style.fill: lightyellow }

start -> env
env -> pre_err: "parse error"
env -> smtp: "ok"
smtp -> ok: "2xx"
smtp -> perm: "5xx (permanent)"
smtp -> trans: "4xx / timeout /\nunknown"
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
