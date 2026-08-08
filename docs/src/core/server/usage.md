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

### Create-and-Subscribe in One Step

`add_and_subscribe_category` is add-only for the category (creates it if missing by
`email_address`, never modifies an existing one) and also subscribes the given account to it.
This is what the Django `/user-sync` webhook uses internally to keep Verteilpunkt mailing-list
assignments in sync, but it can also be called directly for manual assignments:

```bash
spacetime call kommunikationszentrum add_and_subscribe_category \
  <account-id> \
  "member@example.org" \
  "VP Reyerhof" \
  "vp-reyerhof@solawi.example.org" \
  "Verteilpunkt Reyerhof"
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
      "is_admin": false,
      "categories": [
        {
          "name": "VP Reyerhof",
          "email_address": "vp-reyerhof@solawi.example.org",
          "description": "Verteilpunkt Reyerhof"
        }
      ],
      "unsubscribe_category_emails": ["vp-old@solawi.example.org"]
    }
  }'
```

`categories` and `unsubscribe_category_emails` are both optional. Omit them to sync only account
fields, as with earlier payload versions.

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
