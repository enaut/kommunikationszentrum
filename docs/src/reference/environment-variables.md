# Environment Variables Reference

Reference of environment variables supported by Kommunikationszentrum components (`sender`, build scripts, and operational tools).

> [!IMPORTANT]
> **Admin Web Interface & Server Module Configuration**:
> The `admin` frontend and `server` module do **not** read operational settings from runtime environment variables. Configuration settings—such as SpacetimeDB connection URIs, OAuth/OIDC endpoints, visual branding tokens, and localized texts—are configured via compile-time profiles (`profiles/*.toml`). See the [Configuration Profiles Reference](./configuration.md).

---

## Build-Time Environment Variables

These variables control profile selection when building the `admin` frontend or the `server` module.

### `APP_PROFILE`
- **Default**: `"example"`
- **Used by**: `admin/build.rs`, `server/build.rs`
- **Description**: Name of the profile file located in `profiles/` to compile into the binary (e.g. `APP_PROFILE=solawis` resolves to `profiles/solawis.toml`).
- **Examples**:
  ```bash
  APP_PROFILE=solawis dx build -p admin
  APP_PROFILE=solawis spacetime publish -p server kommunikation
  ```

### `APP_CONFIG_PATH`
- **Default**: _(none)_
- **Used by**: `admin/build.rs`, `server/build.rs`
- **Description**: Overrides `APP_PROFILE` with an explicit absolute or relative path to a profile TOML file.
- **Example**: `APP_CONFIG_PATH=/etc/kommunikationszentrum/production.toml`

---

## Runtime Database Configuration (Sender Daemon)

The `sender` background daemon connects to SpacetimeDB to process outbound email queues and record delivery events.

### `SPACETIMEDB_URI`
- **Default**: `http://127.0.0.1:3000`
- **Used by**: `sender`
- **Description**: HTTP / WebSocket connection endpoint for the SpacetimeDB server.
- **Format**: `http://host:port` or `https://host:port`
- **Examples**:
  - Development: `http://localhost:3000`
  - Production: `https://spacetime.example.org`

### `SPACETIMEDB_DATABASE_NAME`
- **Default**: `kommunikation`
- **Used by**: `sender`
- **Description**: SpacetimeDB database / module name to which the sender daemon connects.
- **Format**: String identifier
- **Examples**: `kommunikation`, `kommunikationszentrum`

### `SPACETIMEDB_TOKEN`
- **Default**: _(none)_
- **Used by**: `sender`
- **Description**: SpacetimeDB authentication token for the sender's admin identity. On first start without a token, the sender generates an identity and logs it. Once that identity is granted admin status via `register_admin_identity`, persist the token here to retain credentials across restarts.
- **Format**: SpacetimeDB JWT token string
- **Security**: Must be kept secret; do not commit to version control.

---

## Sender Daemon SMTP Configuration

All outbound mail delivery is performed by the `sender` daemon using `lettre`.

> [!NOTE]
> **SMTP Authentication**:
> - **Mailing List Deliveries**: The sender daemon does **not** take global `SMTP_USERNAME` or `SMTP_PASSWORD` environment variables for mailing list distributions. SMTP authentication credentials (username and app password) are configured per message topic and stored securely in SpacetimeDB (`TopicAppPassword` table), managed via the Admin UI.
> - **Transactional System Emails**: Dedicated system user credentials configured via `SMTP_SYSTEM_USERNAME` and `SMTP_SYSTEM_PASSWORD` are used to authenticate system emails (such as self-service email verification tokens queued in `sender_system_mail_pending`). If either variable is omitted, the sender skips system mail dispatch and logs an error.

### `SMTP_HOST`
- **Default**: _(none — required)_
- **Used by**: `sender`
- **Description**: Hostname or IP address of the outbound SMTP relay server. The daemon will panic on startup if this variable is not set.
- **Format**: Hostname or IP string
- **Examples**: `localhost`, `127.0.0.1`, `mx.example.org`

### `SMTP_PORT`
- **Default**: `465`
- **Used by**: `sender`
- **Description**: Port on which the SMTP relay accepts connections. Common ports: `465` (Implicit TLS/SMTPS), `587` (STARTTLS / submission), `25` (standard relay), `1025` (local test mailcatcher).

### `SMTP_USE_TLS`
- **Default**: `true`
- **Used by**: `sender`
- **Description**: Whether to use TLS when connecting to the SMTP relay.
- **Format**: Boolean (`true` / `false`)

### `SMTP_ACCEPT_INVALID_CERTS`
- **Default**: `false`
- **Used by**: `sender`
- **Description**: When `true`, skips certificate validation (useful for self-signed certificates in development).
- **Warning**: Never enable in production.

### `SMTP_ACCEPT_INVALID_HOSTNAMES`
- **Default**: `false`
- **Used by**: `sender`
- **Description**: When `true`, skips TLS hostname matching validation.
- **Warning**: Never enable in production.

### `SMTP_SYSTEM_USERNAME`
- **Default**: _(none)_
- **Used by**: `sender`
- **Description**: Username for authenticating transactional system emails (e.g. email verification tokens).
- **Format**: Email address or SMTP account username

### `SMTP_SYSTEM_PASSWORD`
- **Default**: _(none)_
- **Used by**: `sender`
- **Description**: Password for authenticating transactional system emails.
- **Security**: Must be kept secret.

### `MAIL_MESSAGE_ID_DOMAIN`
- **Default**: Host extracted from `SPACETIMEDB_URI`, or `localhost`
- **Used by**: `sender`
- **Description**: Domain name used in the right-hand part of generated `Message-ID` headers (`<timestamp-uuid@domain>`).
- **Examples**: `solawis.de`, `mail.example.org`

### `MAIL_UNSUBSCRIBE_BASE_URL`
- **Default**: `<SPACETIMEDB_URI>/v1/database/<DB>/route/mailing-list/unsubscribe`
- **Used by**: `sender`
- **Description**: Endpoint base URL used to construct RFC 8058 `List-Unsubscribe` header URLs.
- **Format**: Full HTTP/HTTPS URL

### `OTLP_ENDPOINT`
- **Default**: `http://localhost:4317`
- **Used by**: `sender`
- **Description**: OpenTelemetry gRPC collector endpoint (e.g. Grafana Alloy, Jaeger, Tempo) for spans and logs. If empty, OTLP export is skipped.
- **Format**: URL (`http://host:port`)

---

## Stalwart Mailserver Integration (Fallback)

SpacetimeDB integrates with the Stalwart mailserver's JMAP API for automated topic mailbox provisioning and domain synchronization.

> [!TIP]
> The primary method for configuring Stalwart is via the **Admin Web UI** (under **Stalwart Mailserver**) or the `set_stalwart_config` reducer, which stores configuration in the database `stalwart_config` table. The environment variables below are used only as fallbacks if the database table is empty.

### `STALWART_JMAP_URL`
- **Default**: _(none)_
- **Used by**: `server`
- **Description**: JMAP endpoint URL of the Stalwart mailserver. If the URL does not end with `/jmap`, it is automatically appended.
- **Examples**: `http://localhost:8093/jmap`, `https://mail.example.org/jmap`

### `STALWART_ADMIN_TOKEN`
- **Default**: _(none)_
- **Used by**: `server`
- **Description**: Stalwart administrative API key secret (created in Stalwart Admin UI under **Management → API Keys**).
- **Format**: Secret token string (e.g. `API_...`)

---

## Observability & Logging

### `RUST_LOG`
- **Default**: `info` (all crates), `sender=info` (sender)
- **Used by**: `sender`, `server`
- **Description**: Tracing and logging filter directives conforming to `tracing-subscriber::EnvFilter`.
- **Values**: `error`, `warn`, `info`, `debug`, `trace`
- **Examples**:
  - General debug: `debug`
  - Sender trace: `sender=trace`
  - Component filter: `sender=debug,spacetimedb_sdk=warn`

---

## External Integration & Testing Variables

### `SPACETIME_WEBHOOK_TOKEN`
- **Default**: _(none)_
- **Used by**: Django backend settings (`solawispielplatz`)
- **Description**: Bearer token sent by Django signal handlers to the `/route/user-sync` endpoint for account synchronization.

### `OIDC_TIMEOUT`
- **Default**: `180`
- **Used by**: `docs/testscripts/spacetime_oidc_login.py`
- **Description**: Timeout in seconds waiting for the local browser callback during the OIDC login helper script.

---

## Sender Daemon Configuration Templates

### Local Development (`.env/.env.sender.example`)
```ini
SPACETIMEDB_URI=http://localhost:3000
SPACETIMEDB_DATABASE_NAME=kommunikation
SPACETIMEDB_TOKEN=

SMTP_HOST=localhost
SMTP_PORT=1025
SMTP_USE_TLS=false
SMTP_ACCEPT_INVALID_CERTS=false
SMTP_ACCEPT_INVALID_HOSTNAMES=false
SMTP_SYSTEM_USERNAME=system@localhost
SMTP_SYSTEM_PASSWORD=
MAIL_MESSAGE_ID_DOMAIN=localhost
MAIL_UNSUBSCRIBE_BASE_URL=http://localhost:3000/v1/database/kommunikation/route/mailing-list/unsubscribe
OTLP_ENDPOINT=http://localhost:4317
RUST_LOG=info
```

### Production Deployment
```ini
SPACETIMEDB_URI=https://spacetimedb.your-domain.com
SPACETIMEDB_DATABASE_NAME=kommunikation
SPACETIMEDB_TOKEN=your-sender-identity-token-here

SMTP_HOST=mail-relay.your-domain.com
SMTP_PORT=465
SMTP_USE_TLS=true
SMTP_ACCEPT_INVALID_CERTS=false
SMTP_ACCEPT_INVALID_HOSTNAMES=false
SMTP_SYSTEM_USERNAME=system@your-domain.com
SMTP_SYSTEM_PASSWORD=your-system-smtp-password-here
MAIL_MESSAGE_ID_DOMAIN=your-domain.com
MAIL_UNSUBSCRIBE_BASE_URL=https://spacetimedb.your-domain.com/v1/database/kommunikation/route/mailing-list/unsubscribe
OTLP_ENDPOINT=http://alloy.internal:4317
RUST_LOG=warn
```

---

## Configuration Validation Checklist

### Sender Daemon Checklist
- [ ] `SMTP_HOST` is set and resolvable.
- [ ] `SPACETIMEDB_URI` is configured and matches the target SpacetimeDB instance.
- [ ] `SPACETIMEDB_TOKEN` is set for production deployments.
- [ ] `SMTP_SYSTEM_USERNAME` and `SMTP_SYSTEM_PASSWORD` are set if self-service verification is enabled.
- [ ] `SMTP_ACCEPT_INVALID_CERTS` and `SMTP_ACCEPT_INVALID_HOSTNAMES` are set to `false` in production.
- [ ] No files containing passwords or production tokens are committed to git.
