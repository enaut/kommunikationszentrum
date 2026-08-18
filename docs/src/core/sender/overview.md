# Sender Daemon Overview

The `sender` crate is the **outbound mail delivery daemon** for the Kommunikationszentrum.
It is a native Rust binary running on the Tokio async runtime that connects to SpacetimeDB,
monitors the delivery pipeline tables, and dispatches outgoing emails over SMTP.

## System Architecture

```d2
{{#include sender-architecture.d2}}
```

## Purpose & Responsibilities

1. **Ingress Fan-Out** — Expands each inbound `MailIngress` record into individual `MailDelivery` rows (one per active subscriber) and enqueues them into `mail_delivery_pending`.
2. **SMTP Submission** — Claims queued deliveries (`mail_delivery_claimed`) with atomic leases and transmits RFC 5322 formatted emails to the outbound SMTP relay.
3. **State Management & Auditing** — Transitions completed deliveries to `mail_delivery_done` (sent/failed/bounced) and writes immutable audit logs to `mail_delivery_events`.
4. **Lease Expiration & Recovery** — Automatically re-claims expired processing or delivery leases in case of worker failure.

The sender daemon is the only component in the system that performs **external network calls** (to the SMTP relay). Everything else is driven by SpacetimeDB's reactive WebSocket subscription model.

## Technology Stack

| Concern | Technology | Notes |
|---|---|---|
| Runtime | Tokio (async Rust) | Multi-threaded async event processing |
| SpacetimeDB SDK | `spacetimedb-sdk 2.8` | Client bindings & table cache |
| SMTP Transport | `lettre 0.11` | Connection pooling & TLS support |
| Tracing & Logs | `opentelemetry` + OTLP | Bridge to Grafana Alloy / Loki / Tempo |
| Log Filtering | `tracing-subscriber` | Configurable via `RUST_LOG` |

## Key Design Principles

### Purely Reactive — Zero Polling Overhead
The daemon does not poll a database on a timer. SpacetimeDB pushes incremental table updates over a WebSocket connection. Subscription callbacks (`on_insert`, `on_update`, `on_delete`) ring a `tokio::sync::Notify` doorbell that wakes the work loop only when actionable work exists.

### Autonomous Connection Pump
The SpacetimeDB connection I/O is spawned into a dedicated background Tokio task (`tokio::spawn(async move { conn.run_async().await })`), ensuring that incoming messages, callbacks, and cache updates are processed immediately even while the main work loop is busy.

### Atomic Claim & Lease Protocol
Work is distributed safely across instances using atomic server-side reducers:
- `claim_next_mail_ingress` grants a 10-minute lease on `MailIngress` (`claim_owner = Identity`, `instance_id = UUID`).
- `claim_next_mail_delivery` grants a 5-minute lease moving a row from `mail_delivery_pending` to `mail_delivery_claimed`.
- Expired leases are automatically recycled by the 60-second server scheduler (`expire_stale_delivery_claims`).

## Source File Map

```
sender/src/
├── main.rs             Entry point, event loop, fan-out logic, delivery dispatch
├── config.rs           SenderConfig — runtime configuration loaded from environment
├── mail.rs             SMTP transport setup, message composition, error classification
└── module_bindings/    Auto-generated SpacetimeDB SDK bindings (do not edit)
    ├── mod.rs          Re-exports all types, table accessors, and reducer stubs
    ├── *_type.rs       Row struct definitions (MailIngress, MailDelivery, etc.)
    ├── *_table.rs      Table accessor traits (iter, find, on_insert, on_update, etc.)
    └── *_reducer.rs    Reducer call stubs
```
