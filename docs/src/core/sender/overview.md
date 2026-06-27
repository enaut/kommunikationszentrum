# Sender Daemon

The `sender` crate is the **outbound mail delivery daemon** for the Kommunikationszentrum.
It is a native Rust binary (Tokio async) that connects to SpacetimeDB, watches the delivery
pipeline tables, and submits outgoing emails over SMTP.

## Purpose

After the SpacetimeDB server module accepts an email from the Stalwart MTA and creates
`MailIngress` records, the sender daemon takes over:

1. **Fan-out** — expands one ingress into individual `MailDelivery` rows (one per subscriber)
2. **SMTP submission** — sends each delivery to the configured SMTP relay
3. **State reporting** — calls reducers to mark deliveries as sent, retry-scheduled, or failed

The sender daemon is the only component in the system that performs **external network calls**
(to the SMTP relay). Everything else is driven by SpacetimeDB's reactive subscription model.

## Technology

| Concern | Technology |
|---|---|
| Runtime | Tokio (async Rust) |
| SpacetimeDB client | `spacetimedb-sdk 2.6` |
| SMTP transport | `lettre 0.11` |
| Observability | OpenTelemetry (OTLP) → traces + logs |
| Log routing | `tracing` + `tracing-subscriber` + `tracing-opentelemetry` |

## Source File Map

```
sender/src/
├── main.rs             Entry point, event loop, fan-out logic, delivery dispatch
├── config.rs           SenderConfig — all runtime configuration from environment variables
├── mail.rs             SMTP transport setup, message composition, error classification
└── module_bindings/    Auto-generated SpacetimeDB SDK bindings
    ├── mod.rs          Re-exports all types, table accessors, and reducer stubs
    ├── *_type.rs       Row struct definitions (MailIngress, MailDelivery, etc.)
    ├── *_table.rs      Table accessor traits
    └── *_reducer.rs    Reducer call stubs
```

## Module Sections

| Section | Description |
|---|---|
| [Configuration](./configuration.md) | All environment variables and their defaults |
| [Control Flow](./control-flow.md) | Startup, event loop, fan-out, delivery, error handling |
| [Usage Guide](./usage.md) | How to run, connect, monitor, and troubleshoot |

## Key Design Decisions

### Purely Reactive — No Polling
The daemon does **not** poll a database on a timer. Instead, SpacetimeDB pushes row-level
updates over a WebSocket subscription. A `tokio::sync::Notify` doorbell is triggered by
`on_insert` and `on_update` callbacks, causing the work loop to run only when there is
something to do.

### In-Flight Tracking
Between calling a claim reducer (e.g. `claim_next_mail_ingress`) and receiving the
server-confirmed state change via the subscription push, there is a small window where the
local cache still shows the old state. An `Arc<Mutex<HashSet<String>>>` tracks IDs that are
actively being processed so the loop does not double-claim work in that window.

### Claim/Lease Protocol
The daemon does not hold a global lock. Each `MailIngress` and `MailDelivery` is claimed
atomically via a reducer call, with a server-side lease expiry (10 min / 5 min). If the
daemon crashes mid-work, the lease expires and another instance can re-claim the job.

### Module Bindings
The `module_bindings/` directory is **auto-generated** by the SpacetimeDB CLI from the
published server module schema:

```bash
spacetime generate --lang rust --out-dir sender/src/module_bindings \
  --server http://localhost:3000 kommunikationszentrum
```

Never edit these files manually — regenerate them after any server schema change.
