# Sender Daemon — Developer Guide

This guide covers how to develop, extend, and test the sender daemon.

---

## Prerequisites

- Rust stable toolchain (2021 edition)
- A running SpacetimeDB instance with the server module published
- An SMTP server for testing (a local relay like Postfix or MailHog works well)
- (Optional) Grafana Alloy / Tempo / Loki for OTLP trace and log collection

---

## Project Structure

```
sender/
├── Cargo.toml                  Binary crate — native Rust target (not WASM)
└── src/
    ├── main.rs                 Entry point, event loop, process_fanout_jobs,
    │                           send_delivery_jobs, send_delivery
    ├── config.rs               SenderConfig struct read from env variables
    ├── mail.rs                 SMTP transport builder, lettre message builder,
    │                           custom header types, subject rewriting
    ├── tracing_util.rs         Deterministic BLAKE3 traceparent synthesis & OTel context
    └── module_bindings/        Auto-generated SpacetimeDB SDK types (do not edit)
        ├── mod.rs
        ├── *_type.rs           Row structs (MailIngress, MailDeliveryClaimed, …)
        ├── *_table.rs          Table accessor traits (iter, find, on_insert, …)
        └── *_reducer.rs        Reducer call stubs
```

---

## Building and Running

```bash
cd sender

# Development build
cargo build

# Run with debug logging
RUST_LOG=sender=debug cargo run

# Production build
cargo build --release
```

---

## Regenerating Bindings

After any change to the server module schema, regenerate the client bindings:

```bash
spacetime generate \
  --lang rust \
  --out-dir sender/src/module_bindings \
  --server http://localhost:3000 \
  kommunikation

cargo build  # verify compilation
```

Never edit `module_bindings/` manually — the next `spacetime generate` will overwrite them.

---

## Architecture Notes

### Event-Driven Work Loop with Safety Tick

The daemon does not use polling as its primary dispatch mechanism. SpacetimeDB pushes table updates over a WebSocket connection. Table callbacks (`on_insert`, `on_update`, `on_delete`) send `Event::Wakeup` through an asynchronous channel (`tokio::sync::mpsc::unbounded_channel`).

A 15-second timer in `tokio::select!` acts as a periodic heartbeat check, ensuring backlogs are processed even if a WebSocket event was missed during network reconnection.

### Background Connection Pump Thread

The SpacetimeDB connection I/O is maintained on a dedicated background OS thread using `connection.run_threaded()`. This ensures that incoming WebSocket frames, callback handlers, and local table cache updates are applied without latency, independent of the Tokio async executor's task queue.

### Atomic Leases and Claim Protocol

Work coordination across instances relies on atomic server-side reducers with lease timeouts:
- `claim_next_mail_ingress` attaches a 10-minute lease (`claim_owner = Identity`, `instance_id = UUID`).
- `claim_next_mail_delivery` moves one row from `mail_delivery_pending` to `mail_delivery_claimed` with a 5-minute lease.
- `schedule_mail_delivery_retry` moves a transiently failed delivery into `mail_delivery_temporary_failed` with a 5-minute backoff delay.
- The 60-second scheduled recycler (`expire_stale_delivery_claims`) automatically recovers abandoned items back to pending queues.

### Message Composition via Lettre

`compose_delivery` in `mail.rs` uses the `lettre::Message::builder()` API:
- Custom mailing-list headers implement `lettre::message::header::Header` via the `custom_header!` macro.
- Outgoing `From` is set to the category list address, while `Reply-To` points to the author.
- Subject lines are parsed through `rewrite_subject()`, normalizing locale-specific reply and forward tags (`Re:`, `Fwd:`, `AW:`, `WG:`, etc.) and ensuring the canonical `[ListName]: ` prefix.
- If email addresses cannot be parsed as valid RFC 5322 addresses, the function falls back to `render_fallback_raw_message()` to ensure mail delivery is not halted by header formatting issues.

---

## Adding a New Feature

### Extending Message Headers

Custom mailing list headers are implemented via the `custom_header!` macro in `mail.rs`:

```rust
custom_header!(XCustomHeader, "X-Custom-Header");
```

To add this header to outbound emails:

```rust
let email = Message::builder()
    .from(...)
    .to(...)
    .header(XCustomHeader("custom-value".to_string()))
    ...
```

If you also want the header in the fallback path, add it to `render_fallback_raw_message()`.

### Handling a New Ingress State

If the server module introduces a new `MailIngress` state:
1. Update `self_owned_ingress_jobs` in `main.rs` if the new state should be processed by workers.
2. Add a corresponding reducer call in `main.rs` to transition out of the new state.
3. Update `control-flow-queue-topology.d2` to document the new transition.

### Adding Observability

All major workflows are annotated with `tracing::instrument`:

```rust
#[instrument(skip(connection, config), fields(ingress_id = %ingress.id, queue_id = tracing::field::Empty))]
fn process_ingress_job(...) { ... }
```

Distributed trace correlation with Stalwart MTA is implemented in `tracing_util.rs`. Any span created after `context_from_traceparent(&traceparent).attach()` automatically inherits the Stalwart SMTP trace context.

---

## Testing

### Integration Testing

1. Start a local SpacetimeDB instance and publish the `kommunikation` module.
2. Start MailHog or Postfix as a local test relay:
   ```bash
   docker run -d -p 1025:1025 -p 8025:8025 mailhog/mailhog
   ```
3. Run the sender pointing at the local relay:
   ```bash
   SPACETIMEDB_URI=http://localhost:3000 \
   SPACETIMEDB_DATABASE_NAME=kommunikation \
   SMTP_HOST=localhost \
   SMTP_PORT=1025 \
   SMTP_USE_TLS=false \
   cargo run -p sender
   ```
4. Post a test MTA hook payload to verify delivery in MailHog (`http://localhost:8025`).

### Unit Testing Pure Helpers

Pure helper functions in `mail.rs` (`rewrite_subject`, `sanitize_header_value`, `message_id_seed`) and `tracing_util.rs` (`traceparent_from_queue_id`, `trace_id_from_traceparent`) can be tested without an active SpacetimeDB connection by writing standard `#[test]` functions and running:

```bash
cargo test -p sender
```

---

## Common Pitfalls

### Missing Admin Permissions on First Startup

The sender daemon must have an admin identity to call claim and complete reducers. If unprovisioned, the sender terminates immediately on startup with:

```
ERROR Sender identity does not have the required admin permissions.
Add this identity (Identity(...)) to admin_identities via the admin interface and add the token (...) to .env and restart the sender.
```

Register the identity in the Admin UI under **Admin Identities**, or via CLI:

```bash
spacetime call kommunikation register_admin_identity "<identity-hex>"
```

Save the generated token to `SPACETIMEDB_TOKEN` in your environment file so the identity is retained across restarts.

### Stale Module Bindings

If the server schema changes and bindings are not regenerated, the sender will fail to compile or misinterpret row fields. Always regenerate bindings after updating `server/`:

```bash
spacetime generate --lang rust --out-dir sender/src/module_bindings --server http://localhost:3000 kommunikation
```

### Double-Claiming & Multi-Instance Safety

Work coordination is handled by atomic server-side reducers with leases. If multiple sender instances run simultaneously, each claim reducer grants a row to only one worker. If no rows are available, the claim reducer returns empty, and the sender waits for the next table notification.
