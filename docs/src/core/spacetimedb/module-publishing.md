# Module Publishing

How to build, publish, and manage the SpacetimeDB module.

## Prerequisites

- SpacetimeDB CLI installed
- Rust toolchain with `wasm32-unknown-unknown` target
- SpacetimeDB running on port 3000
- Environment variables set:
  - `STALWART_ADMIN_TOKEN`
  - `STALWART_JMAP_URL`

Build and publish require these env vars, so they cannot be run by AI agents.

## Build and Publish

```bash
spacetime build -p server kommunikation
spacetime publish -p server kommunikation
```

Use `-c` with publish to clear the database when the schema changes (deletes all data):

```bash
spacetime publish -p server kommunikation -c
```

## Client Bindings

Regenerate bindings after schema or reducer changes:

```bash
spacetimedb-cli generate --lang dioxus -p server -o admin/src/module_bindings/
spacetimedb-cli generate --lang rust -p server -o sender/src/module_bindings/
```

This uses a custom `spacetimedb-cli` and cannot be run by AI agents.

## SpacetimeDB Server

```bash
spacetime start   # localhost:3000
```

## Status and Debugging

```bash
spacetime describe kommunikation
spacetime list
spacetime logs kommunikation
spacetime logs kommunikation --follow
```

```bash
spacetime sql kommunikation "SELECT COUNT(*) FROM account"
spacetime sql kommunikation "SELECT * FROM message_categories LIMIT 10"
spacetime sql kommunikation "SELECT * FROM mta_connection_log ORDER BY timestamp DESC LIMIT 5"
```

## Development Workflow

1. Edit schema or reducers in `server/`
2. Build: `spacetime build -p server kommunikation`
3. Publish: `spacetime publish -p server kommunikation` (add `-c` for schema resets)
4. Regenerate client bindings (see above)
5. Debug with `spacetime logs kommunikation`

# Initializing the module with initial admin credentials

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
  "Sync User Token" \
  '["sync-user"]'
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
