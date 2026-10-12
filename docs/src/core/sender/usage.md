# Usage & Operations Guide

This guide explains how to build, run, inspect, and troubleshoot the sender daemon in development and production environments.

---

## Building

```bash
cd sender

# Development build (unoptimized with symbols)
cargo build

# Production release build
cargo build --release
```

---

## Running

### Development Mode

```bash
# Run with active debug logging against local dev services
SPACETIMEDB_URI=http://localhost:3000 \
SPACETIMEDB_DATABASE_NAME=kommunikationszentrum \
SMTP_HOST=localhost \
SMTP_PORT=1025 \
SMTP_USE_TLS=false \
RUST_LOG=sender=debug \
cargo run -p sender
```

### Production Systemd Service

`/etc/systemd/system/kommunikationszentrum-sender.service`:

```ini
[Unit]
Description=Kommunikationszentrum Outbound Mail Sender Daemon
After=network.target

[Service]
Type=simple
User=kommunikationszentrum
Group=kommunikationszentrum
EnvironmentFile=/etc/kommunikationszentrum/sender.env
ExecStart=/usr/local/bin/sender
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

---

## Initial Identity Provisioning

1. Start the sender daemon for the first time without setting `SPACETIMEDB_TOKEN`.
2. Find the issued identity in the startup log:
   ```
   INFO sender connected as Some(Identity(c200ca120edace75f9ed69590f1a1d44468bf204a97e21365122a56dbc921d95))
   ```
3. Register the sender's identity hex in `admin_identities` (required to call claim reducers):
   ```bash
   spacetime call kommunikationszentrum register_admin_identity "<identity-hex>"
   ```
4. Save the generated auth token to `SPACETIMEDB_TOKEN` in the environment file so the identity is preserved across restarts.

---

## Operational Database Inspection

Inspect queue states using the SpacetimeDB SQL CLI:

```bash
# 1. Inspect Ingress Queue
spacetime sql kommunikation \
  "SELECT id, mail_message_id, topic_email, claim, recipient_count, delivery_count, completed_at FROM mail_ingress"

# 2. Inspect Pending Deliveries (Waiting for Worker)
spacetime sql kommunikation \
  "SELECT delivery_id, ingress_id, attempt_count, last_error, last_updated FROM mail_delivery_pending"

# 3. Inspect Active Worker Leases
spacetime sql kommunikation \
  "SELECT delivery_id, ingress_id, instance_id, lease_expires_at, attempt_count, last_updated FROM mail_delivery_claimed"

# 4. Inspect Temporary Failed Deliveries (Scheduled for Retry)
spacetime sql kommunikation \
  "SELECT delivery_id, ingress_id, next_attempt_at, attempt_count, last_error, last_updated FROM mail_delivery_temporary_failed"

# 5. Inspect Finalized Deliveries
spacetime sql kommunikation \
  "SELECT delivery_id, ingress_id, final_state, attempt_count, finalized_at FROM mail_delivery_done LIMIT 20"

# 6. Inspect Audit Log Events
spacetime sql kommunikation \
  "SELECT id, delivery_id, event_type, attempt_no, details, occurred_at FROM mail_delivery_events LIMIT 25"
```

---

## Troubleshooting Guide

```d2
{{#include troubleshooting-decision-tree.d2}}
```

### Common Diagnostic Checks

| Symptom | Diagnostic Step | Resolution |
|---|---|---|
| Reducers return `Unauthorized` | Query `admin_identities` | Run `spacetime call <db> register_admin_identity "<hex>"` |
| Ingress claimed but not sending | Query `visible_message_topics` & active subscribers | Ensure message topic exists and sender is admin |
| Delivery lease stuck | Check `mail_delivery_claimed` timestamp | Auto-recycles via 60s cron `expire_stale_delivery_claims` |
| SMTP rejects mail (5xx) | Inspect `mail_delivery_events` | Verify SPF/DKIM/From headers match relay policy |
| Transient timeouts (4xx) | Check retry backoff in `mail_delivery_pending` | Auto-retries up to 5 times before marking failed |
