# Sender Overview

> [!WARNING]
> **The `sender` daemon is retained during migration and is planned for retirement only after verification.** See [SpacetimeDB-Native Sender](./spacetimedb-jmap.md) and [`plans/sender-spacetimedb-jmap.md`](../../../../plans/sender-spacetimedb-jmap.md) for the replacement status. The content below describes the daemon path.

The `sender` crate is the **current outbound mail delivery daemon** for the Kommunikationszentrum.
It is a native Rust binary running on the Tokio async runtime that connects to SpacetimeDB,
monitors the delivery pipeline tables, and dispatches outgoing emails over SMTP.

A SpacetimeDB-native JMAP replacement now exists in the `server` module, but remains disabled by
default and has not been verified against a live Stalwart deployment. Keep the daemon and the native
pipeline from processing queues concurrently. See [SpacetimeDB-Native Sender](./spacetimedb-jmap.md)
for the implementation status and rollout constraints.

## System Architecture

```d2
{{#include sender-architecture.d2}}
```

## Purpose & Responsibilities (current daemon)

1. **Ingress Fan-Out** — Expands each inbound `MailIngress` record into individual per-subscriber deliveries and enqueues them into `mail_delivery_pending`, storing full RFC 5322 payloads in `mail_delivery_messages`.
2. **Transactional System Mail Dispatch** — Claims pending system emails (`sender_system_mail_pending`) such as user email verification tokens and dispatches them via dedicated system SMTP credentials with atomic distributed leasing.
3. **SMTP Submission** — Claims queued deliveries (`mail_delivery_claimed`) with atomic leases and transmits RFC 5322 formatted emails over TLS using topic-specific credentials and pooled connections.
4. **State Management & Auditing** — Transitions completed deliveries to `mail_delivery_done` (sent/failed/bounced) or `mail_delivery_temporary_failed` (transient errors) and writes immutable audit logs to `mail_delivery_events`.
5. **Distributed Trace Correlation** — Bridges inbound Stalwart SMTP queue IDs with outbound sender delivery spans via BLAKE3 deterministic W3C traceparents.
6. **Lease Expiration & Recovery** — Automatically re-claims expired processing or delivery leases in case of worker failure.

The sender daemon is the only component in the system that performs **external outbound network calls** (to the SMTP relay). Everything else is driven by SpacetimeDB's reactive WebSocket subscription model.

## Technology Stack (current daemon)

| Concern | Technology | Notes |
|---|---|---|
| Runtime | Tokio (async Rust) | Multi-threaded async event processing |
| SpacetimeDB SDK | `spacetimedb-sdk 2.11` | Client bindings & reactive table cache |
| SMTP Transport | `lettre 0.11` | Tokio Rustls TLS, async send, connection pooling |
| Tracing & Logs | `opentelemetry` + OTLP | Bridge to Grafana Alloy / Loki / Tempo |
| Hashing | `blake3` | Deterministic W3C traceparent derivation from queue IDs |
| Log Filtering | `tracing-subscriber` | Configurable via `RUST_LOG` |

## Source File Map

```
sender/src/
├── main.rs             Entry point, event loop, fan-out logic, delivery dispatch
├── config.rs           SenderConfig — runtime configuration loaded from environment
├── mail.rs             SMTP transport setup, lettre message builder, per-topic credentials
├── tracing_util.rs     BLAKE3 traceparent synthesis from Stalwart queue_id & OTel context
└── module_bindings/    Auto-generated SpacetimeDB SDK bindings (do not edit)
    ├── mod.rs          Re-exports all types, table accessors, and reducer stubs
    ├── *_type.rs       Row struct definitions (MailIngress, MailDeliveryClaimed, etc.)
    ├── *_table.rs      Table accessor traits (iter, find, on_insert, on_update, etc.)
    └── *_reducer.rs    Reducer call stubs
```

> [!NOTE]
> The entire `sender/` crate — including `module_bindings/` — will be removed when the
> SpacetimeDB-native replacement is deployed. Do not add new functionality here.
