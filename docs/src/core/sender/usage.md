# Usage Guide

This page explains how to build, run, monitor, and troubleshoot the sender daemon.

---

## Building

```bash
cd sender
cargo build --release
# Binary: target/release/sender
```

For development:
```bash
cargo build
# Binary: target/debug/sender
```

---

## Running

### Development

```bash
# Set environment variables inline
SPACETIMEDB_URI=http://localhost:3000 \
SPACETIMEDB_DATABASE_NAME=kommunikationszentrum \
SMTP_HOST=localhost \
SMTP_PORT=25 \
SMTP_USE_TLS=false \
RUST_LOG=sender=debug \
cargo run

# Or with a .env file (using a helper like `dotenv` or `direnv`)
dotenv cargo run
```

### Production

```bash
# Recommended: run as a systemd service (see below)
/usr/local/bin/sender
```

---

## First-Time Setup

### 1. Start the Daemon Without a Token

On first launch, SpacetimeDB issues a fresh identity and token. Capture the identity from the
log output:

```
INFO sender connected as Some(Identity { bytes: [...] })
```

### 2. Note the Identity Hex

Query it from SpacetimeDB or use the Admin UI's identity display. Alternatively, call:

```bash
spacetime identity list
```

### 3. Register the Sender as Admin

The sender's identity must be in `admin_identities` to call claim reducers:

```bash
spacetime call kommunikationszentrum register_admin_identity "<64-char-hex>"
```

### 4. Save the Token

Store the issued token in `SPACETIMEDB_TOKEN` so the daemon reconnects with the same identity
on future starts. The token can be retrieved from the SpacetimeDB CLI or the first-startup log.

---

## Systemd Service

```ini
[Unit]
Description=Kommunikationszentrum Sender Daemon
After=network.target

[Service]
Type=simple
User=kommunikationszentrum
EnvironmentFile=/etc/kommunikationszentrum/sender.env
ExecStart=/usr/local/bin/sender
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

`/etc/kommunikationszentrum/sender.env`:
```dotenv
SPACETIMEDB_URI=https://spacetimedb.example.org
SPACETIMEDB_DATABASE_NAME=kommunikationszentrum
SPACETIMEDB_TOKEN=<token>
SMTP_HOST=mail-eu.smtp2go.com
SMTP_PORT=8465
SMTP_USERNAME=myusername
SMTP_PASSWORD=mypassword
SMTP_USE_TLS=true
MAIL_MESSAGE_ID_DOMAIN=example.org
MAIL_UNSUBSCRIBE_BASE_URL=https://spacetimedb.example.org/v1/database/kommunikationszentrum/route/mailing-list/unsubscribe
OTLP_ENDPOINT=http://localhost:4317
RUST_LOG=sender=info
```

---

## Monitoring

### Structured Logs

The daemon emits structured `tracing` events at multiple levels:

| Level | Example events |
|---|---|
| `info` | Startup, shutdown, claiming ingress/delivery jobs, successful sends |
| `warn` | Claim failures, delivery failures, retries |
| `error` | Identity not set, DB pump terminated, fatal errors |
| `trace` | Loop ticks, subscription updates, internal state checks |

Set `RUST_LOG=sender=trace` for maximum verbosity during debugging.

### OpenTelemetry

All spans and log events are exported to the configured `OTLP_ENDPOINT` (default:
`http://localhost:4317`). Grafana Alloy → Tempo (traces) and Loki (logs) is the expected
stack.

Useful span names (from `#[instrument]` annotations):
- `process_fanout_jobs`
- `process_ingress_job`
- `process_subscription_job`
- `process_delivery_jobs`
- `send_delivery`
- `self_owned_ingress_jobs`
- `self_owned_delivery_jobs`

### Checking Pending Work

```bash
# Pending ingresses (not yet claimed)
spacetime sql kommunikationszentrum \
  "SELECT id, state, sender_email, category_email, received_at FROM mail_ingress WHERE state = 'pending'"

# Failed ingresses
spacetime sql kommunikationszentrum \
  "SELECT id, last_error, attempt_count FROM mail_ingress WHERE state = 'failed'"

# Stalled deliveries (retry_scheduled with next_attempt_at in the past)
spacetime sql kommunikationszentrum \
  "SELECT id, state, recipient_email, attempt_count, next_attempt_at FROM mail_deliveries WHERE state = 'retry_scheduled'"

# Recent delivery events
spacetime sql kommunikationszentrum \
  "SELECT delivery_id, event_type, smtp_status_code, details FROM mail_delivery_events ORDER BY occurred_at DESC LIMIT 20"
```

---

## Regenerating Module Bindings

The `module_bindings/` directory is auto-generated from the published server module. Regenerate
it whenever the server schema changes:

```bash
cd sender
spacetime generate \
  --lang rust \
  --out-dir src/module_bindings \
  --server http://localhost:3000 \
  kommunikationszentrum
```

Then rebuild the sender:

```bash
cargo build
```

> **Warning:** Do not edit files inside `module_bindings/` manually. They will be overwritten
> on the next `spacetime generate` run.

---

## Troubleshooting

### Daemon Connects but Doesn't Process Work

- **Check admin registration:** The sender identity must be in `admin_identities`.
  ```bash
  spacetime sql kommunikationszentrum "SELECT * FROM admin_identities"
  ```
- **Check subscriptions:** Run with `RUST_LOG=sender=trace` to see if subscription pushes are
  arriving.
- **Check for stale leases:** If a previous instance died mid-processing, the lease expiry
  (10 min for ingress, 5 min for delivery) must pass before the job is re-claimable.

### SMTP Failures

- Set `RUST_LOG=sender=debug` to see SMTP responses.
- Check `mail_delivery_events` for `smtp_status_code` and `smtp_response` fields.
- Verify `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, and `SMTP_USE_TLS`.

### Deliveries Stuck at `queued` (Never Claimed)

- Verify the daemon is running and connected.
- Check that `sender_mail_deliveries` subscription is active (look for subscription push
  messages at trace level).
- Verify the sender identity is admin.

### "Waiting for unsubscribe token" Retries

If `process_ingress_job` logs this repeatedly:
1. The `ensure_subscription_unsubscribe_token` reducer was called but the token hasn't
   propagated back via the subscription yet.
2. This is normally transient — the ingress will be retried and the token will be present.
3. If it persists, query the token table:
   ```bash
   spacetime sql kommunikationszentrum \
     "SELECT * FROM subscription_unsubscribe_tokens WHERE subscription_id = <id>"
   ```
