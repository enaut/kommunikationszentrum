# Using the Server Module

This page explains how to interact with the SpacetimeDB server module as an operator or
integrator — publishing the module, managing webhook tokens, calling reducers, and testing
the HTTP endpoints.

---

## Publishing the Module

The module must be published to a running SpacetimeDB instance before any other component can
connect.

```bash
# From the repository root
cd server

# Build and publish (replace <module-name> with your chosen name)
spacetime publish --server http://localhost:3000 kommunikationszentrum
```

On first publish, the `init` reducer runs and seeds the publisher's identity as an admin.

### Re-publishing After Changes

```bash
spacetime publish --server http://localhost:3000 kommunikationszentrum
```

SpacetimeDB applies schema migrations automatically. Existing data is preserved where possible.

---

## Managing Webhook Tokens

External systems authenticate with the module's HTTP endpoints using webhook tokens. Tokens
are created by an admin via the SpacetimeDB CLI or the Admin UI.

### Create a Token

The **plaintext** token must be hashed client-side before calling the reducer. The module
never receives the plaintext.

```bash
# 1. Generate a random token
TOKEN=$(openssl rand -hex 32)
echo "Save this token securely: $TOKEN"

# 2. Hash it with BLAKE3 (requires b3sum or the spacetime CLI)
TOKEN_HASH=$(echo -n "$TOKEN" | b3sum --no-names)
# Or, using the Admin UI token management page

# 3. Register the hash with the module
spacetime call kommunikationszentrum create_webhook_token \
  "$TOKEN_HASH" \
  "Stalwart MTA hook" \
  '["mta-hook"]'
```

### Revoke a Token

```bash
spacetime call kommunikationszentrum revoke_webhook_token "$TOKEN_HASH"
```

### Available Permissions

| Permission | Endpoint |
|---|---|
| `mta-hook` | `POST /mta-hook` |
| `sync-user` | `POST /user-sync` |

---

## Managing Admin Identities

### Grant Admin Status

```bash
# Get the target user's SpacetimeDB identity hex
spacetime call kommunikationszentrum register_admin_identity "<64-char-hex>"
```

### Revoke Admin Status

```bash
spacetime call kommunikationszentrum unregister_admin_identity "<64-char-hex>"
```

---

## Managing Mailing List Categories

### Create a Category (without Stalwart provisioning)

```bash
spacetime call kommunikationszentrum add_message_category \
  "SoLaWi News" \
  "news@solawi.example.org" \
  "Weekly newsletter for members"
```

### Create a Category (with Stalwart mailbox provisioning)

Use the Admin UI, or call the procedure via a SpacetimeDB client. The `provision_message_category`
procedure requires the module to be built with `STALWART_JMAP_URL` and `STALWART_ADMIN_TOKEN`
set as compile-time environment variables.

### Remove a Category

```bash
spacetime call kommunikationszentrum remove_message_category <category-id>
```

---

## Testing the HTTP Endpoints

### MTA Hook (CONNECT stage)

```bash
curl -X POST "http://localhost:3000/v1/database/kommunikationszentrum/route/mta-hook" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "context": {
      "stage": "connect",
      "client": { "ip": "1.2.3.4", "ptr": null, "helo": null },
      "server": { "ip": "10.0.0.1", "port": 25, "hostname": "mail.example.org" },
      "queue": null,
      "protocol": "smtp"
    },
    "envelope": null,
    "message": null
  }'
```

### MTA Hook (DATA stage)

See `docs/testscripts/` for complete example payloads covering all stages.

### User Sync

```bash
curl -X POST "http://localhost:3000/v1/database/kommunikationszentrum/route/user-sync" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "action": "upsert",
    "user": {
      "mitgliedsnr": 42,
      "name": "Alice Example",
      "email": "alice@example.org",
      "is_active": true,
      "is_admin": false
    }
  }'
```

### One-Click Unsubscribe

```bash
# TOKEN is the value from subscription_unsubscribe_tokens.token
curl -X POST \
  "http://localhost:3000/v1/database/kommunikationszentrum/route/mailing-list/unsubscribe?token=$UNSUB_TOKEN" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "List-Unsubscribe=One-Click"
```

---

## Viewing Logs

```bash
# Tail the module logs
spacetime logs kommunikationszentrum --follow

# Dump MTA processing logs to server output
spacetime call kommunikationszentrum dump_mta_logs_to_server_logs
spacetime logs kommunikationszentrum | tail -n 50
```

---

## Querying the Database

```bash
# List all accounts
spacetime sql kommunikationszentrum "SELECT * FROM account"

# List active subscriptions
spacetime sql kommunikationszentrum "SELECT * FROM subscriptions WHERE active = true"

# List pending mail ingress records
spacetime sql kommunikationszentrum "SELECT id, state, sender_email, category_email FROM mail_ingress WHERE state = 'pending'"

# List recent delivery events
spacetime sql kommunikationszentrum "SELECT * FROM mail_delivery_events ORDER BY occurred_at DESC LIMIT 20"
```
