# Module Publishing

How to build, publish, and manage the SpacetimeDB module.

## Prerequisites

- SpacetimeDB CLI installed
- Rust toolchain with `wasm32-unknown-unknown` target
- SpacetimeDB running on port 3000
- Configuration profile in `profiles/` (see [Configuration Profiles](../../reference/configuration.md))
- Optional: Stalwart credentials (can be set at publish time or configured post-publish via the Admin UI; see [Stalwart MTA Setup](../../email/stalwart-setup.md))

## Build and Publish

Publish through the profile-aware wrapper. It reads `profiles/$APP_PROFILE/profile.toml`, uses the profile's module name, and passes `APP_PROFILE` to `spacetime publish` so the module build uses that profile. It sets the profile's OIDC issuer as a SpacetimeDB module environment value. Without `--server`, SpacetimeDB CLI publishes to its local default server; pass `--server <url-or-name>` to publish elsewhere. `APP_PROFILE` defaults to `example`. The wrapper sets all six declared module environment variables from the profile: `OIDC_ISSUER_URL` from `[oauth].issuer_url`; `JMAP_DISPATCH_ENABLED`, `SYSTEM_JMAP_USERNAME`, and `SYSTEM_JMAP_PASSWORD` from `[mail]`; `MESSAGE_ID_DOMAIN` from the hostname in `[server].frontend_base_url`; and `MAIL_UNSUBSCRIBE_BASE_URL` from `[spacetimedb].uri`. Blank mail credentials are exported as empty strings, so the profile is authoritative on each publish. Store production credentials only in an ignored custom profile.

The Zed publish task sources `.env/.env.oidc` to select `APP_PROFILE`; the six module values are then loaded from that profile and replace any matching shell values.

```bash
APP_PROFILE=my-org python3 scripts/publish_spacetime.py
```

Pass additional `spacetime publish` options after the wrapper command. For example, use `-c` to clear the database when a schema change requires it (deletes all data):

```bash
APP_PROFILE=my-org python3 scripts/publish_spacetime.py -c
```

Direct `spacetime publish` still builds the server module, but does not copy `[oauth].issuer_url` from the application profile into the published module environment.

## Client Bindings

Regenerate bindings after schema or reducer changes:

```bash
spacetimedb-cli generate --lang=dioxus -p server -o admin/src/module_bindings/
```

The command compiles the module and generates client bindings from its schema. Verify the generated files after schema changes; binding generation does not publish to a database.

## SpacetimeDB Server

```bash
spacetime start   # localhost:3000
```

## Managing Module Environment Secrets

SpacetimeDB v2.12+ supports typed environment secrets declared in the module via `#[spacetimedb::env]`. Secrets are set at publish time and persist across subsequent publishes. They are readable by module code via `ctx.env` but are never exposed to clients.

### Change profile-backed values

Edit the corresponding fields in the selected `profile.toml`, then publish with `scripts/publish_spacetime.py`. The wrapper supplies all six module values on every publish; blank JMAP credentials are supplied as empty strings and therefore clear previously stored values. The application profile is the source of truth.

Do not use `--env-only` to manage these six values because it bypasses the profile. A direct `spacetime publish --env-only` change can be replaced by the next wrapper publish.

### List declared environment keys

```bash
spacetime env list kommunikation
```

### Read the current value of a key

```bash
spacetime env get kommunikation JMAP_DISPATCH_ENABLED
```

`spacetime env list` and `spacetime env get` display values. Do not share their output if it contains secrets; do not read back a password just to verify it.

---

## Status and Debugging

```bash
spacetime describe kommunikation
spacetime list
spacetime logs kommunikation
spacetime logs kommunikation --follow
```

```bash
spacetime sql kommunikation "SELECT COUNT(*) FROM account"
spacetime sql kommunikation "SELECT * FROM message_topics LIMIT 10"
spacetime sql kommunikation "SELECT * FROM received_message LIMIT 5"
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

The **plaintext** token must be hashed client-side before calling the reducer. The module never receives or stores the plaintext — only the BLAKE3 hash is registered.

#### Option 1: Via Admin Web UI (Recommended)

The Admin UI allows generating and managing webhook tokens directly in the browser:

![Admin UI Webhook Token Management](../../email/img/admin-mta-token-creation.png)

1. Open the Admin UI and navigate to the **Webhook Tokens** view.
2. Enter a **Label** (e.g. `Stalwart` for MTA hooks, or `Sync User Token` for Django).
3. Enter or select the required **Permission** (`mta-hook` or `sync-user`).
4. Click **+ Token generieren** to generate a cryptographically random 32-byte token in the browser.
5. Click **Kopieren** to copy the plaintext token and save it securely.
6. Click **Token erstellen**. The browser computes the BLAKE3 hash client-side and registers only the hash with the module via `create_webhook_token`.
7. Existing tokens and permissions are displayed in the list below with a delete button to revoke them.

> [!NOTE]
> The plaintext token is displayed only once in the browser upon generation. It cannot be recovered from the database.

---

#### Option 2: Via SpacetimeDB CLI

Alternatively, generate and hash the token manually using CLI tools:

**For Stalwart MTA hook (`mta-hook` permission):**

```bash
# 1. Generate a random token
TOKEN=$(openssl rand -hex 32)
echo "Save this token securely: $TOKEN"

# 2. Hash it with BLAKE3 (requires b3sum or the spacetime CLI)
TOKEN_HASH=$(echo -n "$TOKEN" | b3sum --no-names)

# 3. Register the hash with the module
spacetime call kommunikation create_webhook_token \
  "$TOKEN_HASH" \
  "Stalwart MTA hook" \
  '["mta-hook"]'
```

**For Django user sync (`sync-user` permission):**

```bash
# 1. Generate a random token
TOKEN=$(openssl rand -hex 32)
echo "Save this token securely: $TOKEN"

# 2. Hash it with BLAKE3
TOKEN_HASH=$(echo -n "$TOKEN" | b3sum --no-names)

# 3. Register the hash with the module
spacetime call kommunikation create_webhook_token \
  "$TOKEN_HASH" \
  "Sync User Token" \
  '["sync-user"]'
```

### Revoke a Token

```bash
spacetime call kommunikation revoke_webhook_token "$TOKEN_HASH"
```

### Available Permissions

| Permission | Endpoint |
|---|---|
| `mta-hook` | `POST /mta-hook` |
| `sync-user` | `POST /user-sync` |

---

## Managing Admin Identities

Admin identities have elevated privileges to execute admin-only reducers and procedures (such as managing webhook tokens, domain sync, topic creation, and recipient management).

### Option 1: Via Admin Web UI (Recommended)

Manage identities visually from the **Admin-Identitäten** card:

![Admin Identities Management](../../email/img/admin-identity-management.png)

1. Navigate to the **Admin-Identitäten** view.
2. In the **Identity Hex (64 Zeichen)** input field, paste the 64-character hexadecimal SpacetimeDB identity of the user or service.
3. Click **+ Hinzufügen** to register the identity as an admin.
4. Existing admin identities are listed with their active count. To revoke admin privileges from an identity, click the remove icon next to it.

---

### Option 2: Via SpacetimeDB CLI

**Grant Admin Status:**

```bash
# Get the target user's SpacetimeDB identity hex (64 hex characters)
spacetime call kommunikation register_admin_identity "<64-char-hex>"
```

**Revoke Admin Status:**

```bash
spacetime call kommunikation unregister_admin_identity "<64-char-hex>"
```
