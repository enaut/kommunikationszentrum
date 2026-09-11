# Sender Daemon Overview

The `sender` crate is the **outbound mail delivery daemon** for the Kommunikationszentrum.
It is a native Rust binary running on the Tokio async runtime that connects to SpacetimeDB,
monitors the delivery pipeline tables, and dispatches outgoing emails over SMTP.

## System Architecture

```d2
{{#include sender-architecture.d2}}
```

## Purpose & Responsibilities

1. **Ingress Fan-Out** — Expands each inbound `MailIngress` record into individual per-subscriber deliveries and enqueues them into `mail_delivery_pending`, storing full RFC 5322 payloads in `mail_delivery_messages`.
2. **Transactional System Mail Dispatch** — Claims pending system emails (`sender_system_mail_pending`) such as user email verification tokens and dispatches them via dedicated system SMTP credentials with atomic distributed leasing.
3. **SMTP Submission** — Claims queued deliveries (`mail_delivery_claimed`) with atomic leases and transmits RFC 5322 formatted emails over TLS using category-specific credentials and pooled connections.
4. **State Management & Auditing** — Transitions completed deliveries to `mail_delivery_done` (sent/failed/bounced) or `mail_delivery_temporary_failed` (transient errors) and writes immutable audit logs to `mail_delivery_events`.
5. **Distributed Trace Correlation** — Bridges inbound Stalwart SMTP queue IDs with outbound sender delivery spans via BLAKE3 deterministic W3C traceparents.
6. **Lease Expiration & Recovery** — Automatically re-claims expired processing or delivery leases in case of worker failure.

The sender daemon is the only component in the system that performs **external outbound network calls** (to the SMTP relay). Everything else is driven by SpacetimeDB's reactive WebSocket subscription model.

## Technology Stack

| Concern | Technology | Notes |
|---|---|---|
| Runtime | Tokio (async Rust) | Multi-threaded async event processing |
| SpacetimeDB SDK | `spacetimedb-sdk 2.10` | Client bindings & reactive table cache |
| SMTP Transport | `lettre 0.11` | Tokio Rustls TLS, async send, connection pooling |
| Tracing & Logs | `opentelemetry` + OTLP | Bridge to Grafana Alloy / Loki / Tempo |
| Hashing | `blake3` | Deterministic W3C traceparent derivation from queue IDs |
| Log Filtering | `tracing-subscriber` | Configurable via `RUST_LOG` |

## Key Design Principles

### Reactive Event Loop with Safety Tick
SpacetimeDB pushes incremental table updates over a WebSocket connection. Subscription callbacks (`on_insert`, `on_update`, `on_delete`) send `Event::Wakeup` across an asynchronous channel (`tokio::sync::mpsc::unbounded_channel`) that wakes the work loop immediately when actionable work exists. A 15-second fallback poll timer in `tokio::select!` ensures the loop recovers smoothly if a network blip ever delays an event.

### Dedicated Connection Pump Thread
The SpacetimeDB connection I/O is spawned onto a dedicated background OS thread (`connection.run_threaded()`), ensuring incoming frames, subscription callbacks, and table cache updates are processed immediately even when the Tokio runtime is heavily loaded.

### Fail-Fast Admin Identity Verification
On startup, once the initial database subscription snapshot is applied, the sender checks its own identity against `visible_admin_identities`. If the identity is not an authorized administrator, the sender logs clear remediation instructions and emits `Event::FatalError`, terminating cleanly rather than repeatedly failing claim reducers at runtime.

### Atomic Claim & Lease Protocol
Work is distributed safely across instances using atomic server-side reducers:
- `claim_next_mail_ingress` grants a 10-minute lease on `MailIngress` (`claim_owner = Identity`, `instance_id = UUID`).
- `claim_system_mail` grants a 5-minute lease on `system_mail_pending` (`instance_id = UUID`, `claimed_at = Timestamp`).
- `claim_next_mail_delivery` grants a 5-minute lease moving a row from `mail_delivery_pending` to `mail_delivery_claimed`.
- Transient SMTP failures are moved to `mail_delivery_temporary_failed` with a 5-minute retry delay.
- Expired leases are automatically recycled by the 60-second server scheduler (`expire_stale_delivery_claims`) or lease timeouts.

## Source File Map

```
sender/src/
├── main.rs             Entry point, event loop, fan-out logic, delivery dispatch
├── config.rs           SenderConfig — runtime configuration loaded from environment
├── mail.rs             SMTP transport setup, lettre message builder, per-category credentials
├── tracing_util.rs     BLAKE3 traceparent synthesis from Stalwart queue_id & OTel context
└── module_bindings/    Auto-generated SpacetimeDB SDK bindings (do not edit)
    ├── mod.rs          Re-exports all types, table accessors, and reducer stubs
    ├── *_type.rs       Row struct definitions (MailIngress, MailDeliveryClaimed, etc.)
    ├── *_table.rs      Table accessor traits (iter, find, on_insert, on_update, etc.)
    └── *_reducer.rs    Reducer call stubs
```
