# Sender Daemon — Developer Guide

This guide covers how to develop, extend, and test the sender daemon.

---

## Prerequisites

- Rust stable toolchain
- A running SpacetimeDB instance with the server module published
- An SMTP server for testing (a local relay like Postfix or a service like MailHog works well)
- (Optional) Grafana Alloy for OTLP trace/log collection

---

## Project Structure

```
sender/
├── Cargo.toml                  Binary crate — native Rust target (not WASM)
└── src/
    ├── main.rs                 Entry point, event loop, process_fanout_jobs,
    │                           process_delivery_jobs, send_delivery, etc.
    ├── config.rs               SenderConfig struct read from env variables
    ├── mail.rs                 SMTP transport, message composition, error classification
    └── module_bindings/        Auto-generated SpacetimeDB SDK types (do not edit)
        ├── mod.rs
        ├── *_type.rs           Row structs (MailIngress, MailDelivery, Subscription, …)
        ├── *_table.rs          Table accessor traits (iter, find, on_insert, on_update, …)
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

After any change to the server module schema, regenerate the bindings:

```bash
spacetime generate \
  --lang rust \
  --out-dir sender/src/module_bindings \
  --server http://localhost:3000 \
  kommunikationszentrum

cargo build  # verify compilation
```

Never edit `module_bindings/` manually — the next `spacetime generate` will overwrite them.

---

## Architecture Notes

### Why No Polling Timer?

The daemon is entirely event-driven. SpacetimeDB pushes updates over WebSocket whenever
subscribed rows change. The `Notify` doorbell wakes the work loop only when there is data to
process, keeping CPU usage near zero when idle.

### The 50 ms Sleep

After calling `claim_next_mail_ingress()` or `claim_next_mail_delivery()`, the daemon sleeps
50 ms before checking the local cache. This accounts for the round-trip time between the
reducer call and the subscription push arriving. The sleep is a pragmatic compromise — a
cleaner approach would be to use the `on_update` callback as the sole trigger, but the current
pattern keeps the code straightforward.

### In-Flight Sets vs. Subscription Cache

The `in_flight_ingresses` / `in_flight_deliveries` `HashSet`s exist to bridge the gap between
*calling a reducer* and *receiving the resulting state change*. Without them, the claim loop
might attempt to re-process the same job before SpacetimeDB's push arrives.

The sets are maintained as follows:
- **Insert** — immediately before calling `process_ingress_job` / `send_delivery`.
- **Remove** — in the `on_update` callback, when the row transitions out of `processing` /
  `sending` state.

### Message Composition

`compose_delivery` in `mail.rs` does not use a MIME library — it constructs raw RFC 5322
messages by concatenating header lines and the body. This keeps the output simple and
predictable, but means:

- No MIME multipart support (the body is passed verbatim from `ingress.body_raw`).
- No base64 encoding of the body.
- Mailing list headers are always prepended, replacing the original `From` with the list
  address and setting `Reply-To` to the original sender.

If MIME support is needed in the future, the `lettre::Message` builder API or the `mail-builder`
crate can be integrated.

---

## Adding a New Feature

### Extending the Message Headers

Edit `compose_delivery` in `mail.rs`. Add the new `(name, value)` tuple to the `headers`
vector. The `render_raw_message` function will include it automatically.

### Handling a New Ingress State

If the server module adds a new `MailIngress` state:

1. Add the state constant in `main.rs` (mirroring the server module's constants).
2. Update `self_owned_ingress_jobs` if the new state should be claimable.
3. Update `claimable_ingress` filter logic in `process_fanout_jobs` if needed.
4. Add a corresponding reducer call in `main.rs` to transition out of the new state.

### Adding Observability

All major functions are annotated with `#[instrument]`. To add a new span field:

```rust
#[instrument(skip(connection), fields(my_field = %value))]
fn my_function(connection: &DbConnection, value: &str) { … }
```

Dynamic field values can be set inside the function:

```rust
tracing::Span::current().record("ingress_id", &ingress.id.as_str());
```

---

## Testing

### Unit Tests

`mail.rs` functions (`compose_delivery`, `rewrite_subject`, `render_raw_message`) can be tested
without a SpacetimeDB connection:

```bash
cargo test
```

### Integration Testing

1. Start a local SpacetimeDB instance and publish the server module.
2. Use a local SMTP server (e.g. MailHog or Postfix) as the relay.
3. POST a test MTA hook payload to the module's `/mta-hook` endpoint.
4. Verify `mail_ingress` rows appear and the sender daemon processes them.
5. Check MailHog's web UI for received emails.

```bash
# Start MailHog (captures outbound mail, no real delivery)
docker run -d -p 1025:1025 -p 8025:8025 mailhog/mailhog

# Configure sender to point at MailHog
SMTP_HOST=localhost SMTP_PORT=1025 SMTP_USE_TLS=false cargo run
```

### Manual Triggering

To test without a real Stalwart MTA, insert a `MailIngress` row directly via the SpacetimeDB
CLI (using the module's `upsert_mail_ingress` helper indirectly by calling `handle_mta_hook`
with a crafted DATA-stage payload):

```bash
curl -X POST \
  "http://localhost:3000/v1/database/kommunikationszentrum/route/mta-hook" \
  -H "Authorization: Bearer <mta-hook-token>" \
  -H "Content-Type: application/json" \
  -d @docs/testscripts/data_stage_payload.json
```

---

## Common Pitfalls

### Sender Identity Not Admin

The most common issue on first startup: the sender daemon connects but all `claim_*` reducer
calls return "Unauthorized". Solution:

```bash
# Find the sender's identity in the logs:
# INFO sender connected as Some(Identity { bytes: [..] })
# Then register it:
spacetime call kommunikationszentrum register_admin_identity "<hex>"
```

### Stale Module Bindings

If the server module schema changes and bindings are not regenerated, the sender will fail to
compile or will misinterpret row data. Always regenerate bindings after a server schema change:

```bash
spacetime generate --lang rust --out-dir sender/src/module_bindings \
  --server http://localhost:3000 kommunikationszentrum
```

### Double-Claiming

If two sender instances run simultaneously against the same module, they will compete for
claims. The server module's claim protocol is safe — only one instance can hold a lease at a
time — but the 50 ms sleep window means both might attempt to call the same claim reducer
nearly simultaneously. The server will grant the claim to only one; the other receives no
owned rows after the sleep and moves on. This is safe but noisy in logs.

### Body Encoding

The body is taken verbatim from `ingress.body_raw` (set during the DATA hook). If the original
email used quoted-printable or base64 `Content-Transfer-Encoding`, those bytes are passed
through unchanged. If the recipient's mail client or relay does not handle the encoding, the
email may display incorrectly. Proper MIME passthrough is a known limitation.
