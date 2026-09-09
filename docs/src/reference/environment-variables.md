# Environment Variables Reference

Complete reference of all environment variables supported by Kommunikationszentrum components (`admin`, `sender`, `server`, and integration scripts).

---

## Core Database Configuration

### `SPACETIMEDB_URI`
- **Default**: `http://localhost:3000` (admin), `http://127.0.0.1:3000` (sender)
- **Used by**: `admin`, `sender`
- **Description**: HTTP / WebSocket connection endpoint for the SpacetimeDB server.
- **Format**: `http://host:port` or `https://host:port`
- **Examples**:
  - Development: `http://localhost:3000`
  - Production: `https://spacetime.example.org`

### `SPACETIMEDB_MODULE_NAME`
- **Default**: `kommunikation`
- **Used by**: `admin`
- **Description**: SpacetimeDB database/module name for client connections and subscriptions.
- **Format**: String identifier
- **Examples**: `kommunikation`, `kommunikationszentrum`

### `SPACETIMEDB_DATABASE_NAME`
- **Default**: `kommunikation`
- **Used by**: `sender`
- **Description**: SpacetimeDB database/module name to which the sender daemon connects.
- **Format**: String identifier
- **Examples**: `kommunikation`, `kommunikationszentrum`

### `SPACETIMEDB_TOKEN`
- **Default**: _(none)_
- **Used by**: `sender`
- **Description**: SpacetimeDB authentication token for the sender's admin identity. On first start without a token, the sender generates an identity and logs it. Once that identity is granted admin status via `register_admin_identity`, persist the token here to retain credentials across restarts.
- **Format**: SpacetimeDB JWT token string
- **Security**: Must be kept secret; do not commit to version control.

---

## Authentication & OAuth Configuration

### `DJANGO_BASE_URL`
- **Default**: `http://127.0.0.1:8000`
- **Used by**: `admin`, `server` (compile-time fallback via `option_env!`)
- **Description**: Base URL of the Django backend serving as the OAuth 2.0 / OpenID Connect provider (solawispielplatz).
- **Format**: `http://host:port` or `https://host:port`
- **Examples**:
  - Development: `http://127.0.0.1:8000`
  - Production: `https://auth.example.org`

### `OIDC_ISSUER_URL`
- **Default**: `{DJANGO_BASE_URL}/o` (e.g. `http://127.0.0.1:8000/o`)
- **Used by**: `admin`
- **Description**: OIDC discovery issuer URL. Usually `DJANGO_BASE_URL` appended with `/o` for Django OAuth Toolkit.
- **Format**: `http://host:port/path` or `https://host:port/path`

### `OIDC_CLIENT_ID`
- **Default**: `admin-app`
- **Used by**: `admin`, `docs/testscripts/spacetime_oidc_login.py`
- **Description**: OAuth 2.0 client identifier registered in Django OAuth Toolkit.
- **Format**: String identifier
- **Security**: Should be unique per deployment environment.
- **Examples**: `admin-app`, `kommunikationszentrum-prod`

### `ADMIN_REDIRECT_URI`
- **Default**: `http://127.0.0.1:8080/callback`
- **Used by**: `admin`
- **Description**: Callback redirect URL for the OAuth authorization code flow in the admin UI.
- **Format**: Full URL with protocol
- **Requirements**: Must be registered in Django's redirect URIs.

### `OAUTH_SCOPES`
- **Default**: `openid profile email`
- **Used by**: `admin`
- **Description**: Space-separated list of OAuth scopes requested during login.
- **Format**: `scope1 scope2 scope3`
- **Requirements**: `openid` must be included.

### `OIDC_TIMEOUT`
- **Default**: `180`
- **Used by**: `docs/testscripts/spacetime_oidc_login.py`
- **Description**: Timeout in seconds waiting for the local browser callback during the OIDC login helper script.
- **Format**: Integer (seconds)

---

## Sender Daemon Configuration

All outbound mail delivery is performed by the `sender` daemon using `lettre`.

> [!NOTE]
> **SMTP Authentication**: The sender daemon does **not** take global `SMTP_USERNAME` or `SMTP_PASSWORD` environment variables. SMTP authentication credentials (username and app password) are configured per message category and stored securely in SpacetimeDB (`CategoryAppPassword` table), managed via the Admin UI.

### `SMTP_HOST`
- **Default**: _(none — required)_
- **Used by**: `sender`
- **Description**: Hostname or IP address of the outbound SMTP relay server. The daemon will panic on startup if this variable is not set.
- **Format**: Hostname or IP string
- **Examples**: `localhost`, `127.0.0.1`, `mail-eu.smtp2go.com`

### `SMTP_PORT`
- **Default**: `465`
- **Used by**: `sender`
- **Description**: TCP port of the outbound SMTP relay (e.g. `25` for unauthenticated local relay, `465` for implicit TLS, `587` for STARTTLS). Ports `143` and `993` are rejected because they are IMAP ports.
- **Format**: Integer port number

### `SMTP_USE_TLS`
- **Default**: `true`
- **Used by**: `sender`
- **Description**: Whether to use TLS encryption when connecting to the SMTP relay.
- **Format**: Boolean (`true` or `false`)

### `SMTP_ACCEPT_INVALID_CERTS`
- **Default**: `false`
- **Used by**: `sender`
- **Description**: Accept expired or self-signed certificates when TLS is enabled. Useful for local development relays (e.g. self-signed Stalwart or MailHog).
- **Format**: Boolean (`true` or `false`)

### `SMTP_ACCEPT_INVALID_HOSTNAMES`
- **Default**: `false`
- **Used by**: `sender`
- **Description**: Accept mismatched certificate hostnames when TLS is enabled. Useful for local testing against `localhost` or IP addresses.
- **Format**: Boolean (`true` or `false`)

### `MAIL_MESSAGE_ID_DOMAIN`
- **Default**: Derived from the host of `SPACETIMEDB_URI`, falling back to `solawis.de`
- **Used by**: `sender`
- **Description**: Fully qualified domain name used to generate unique `Message-ID` headers for outbound emails.
- **Format**: Domain string
- **Examples**: `solawis.de`, `mail.example.org`

### `MAIL_UNSUBSCRIBE_BASE_URL`
- **Default**: `{SPACETIMEDB_URI}/v1/database/{SPACETIMEDB_DATABASE_NAME}/route/mailing-list/unsubscribe`
- **Used by**: `sender`
- **Description**: Base URL for generating RFC 8058 one-click unsubscribe links included in outbound headers and footer.
- **Format**: Full HTTP/HTTPS URL

### `OTLP_ENDPOINT`
- **Default**: `http://localhost:4317`
- **Used by**: `sender`
- **Description**: OpenTelemetry gRPC collector endpoint (e.g. Grafana Alloy, Jaeger, Tempo) for spans and logs. If empty, OTLP export is skipped.
- **Format**: URL (`http://host:port`)

---

## Stalwart Mailserver Integration (Fallback)

SpacetimeDB integrates with the Stalwart mailserver's JMAP API for automated category mailbox provisioning and domain synchronization.

> [!TIP]
> The primary and recommended method for configuring Stalwart is via the **Admin Web UI** (under **Stalwart Mailserver**) or the `set_stalwart_config` reducer, which stores configuration in the database `stalwart_config` table. The environment variables below are used as fallbacks if the database table is empty.

### `STALWART_JMAP_URL`
- **Default**: _(none)_
- **Used by**: `server`
- **Description**: JMAP endpoint URL of the Stalwart mailserver. If the URL does not end with `/jmap`, it is automatically appended.
- **Format**: `http://host:port/jmap` or `https://host:port/jmap`
- **Examples**: `http://localhost:8093/jmap`, `http://fedora.fritz.box:8093/jmap`

### `STALWART_ADMIN_TOKEN`
- **Default**: _(none)_
- **Used by**: `server`
- **Description**: Stalwart administrative API key secret (created in Stalwart Admin UI under **Management → API Keys**).
- **Format**: Secret token string (e.g. `API_...`)

---

## Observability & Logging

### `RUST_LOG`
- **Default**: `info` (all crates), `sender=info` (sender)
- **Used by**: `admin`, `sender`, `server`
- **Description**: Tracing and logging filter directives conforming to `tracing-subscriber::EnvFilter`.
- **Values**: `error`, `warn`, `info`, `debug`, `trace`
- **Examples**:
  - General debug: `debug`
  - Sender trace: `sender=trace`
  - Component filter: `sender=debug,spacetimedb_sdk=warn`

---

## Test & Integration Script Variables

These variables are used by helper and integration test scripts in `docs/testscripts/`:

### `WEBHOOK_TOKEN`
- **Default**: _(none — required by scripts)_
- **Used by**: `test-mta-hooks.sh`, `test-user-sync.sh`
- **Description**: Bearer token used to authenticate against SpacetimeDB embedded HTTP routes (`/route/mta-hook` and `/route/user-sync`). The token is generated via the Admin UI or CLI and validated against the BLAKE3 hash stored in `webhook_token`.
- **Format**: 32-byte hex token string

### `SPACETIME_HOST`
- **Default**: `http://localhost:3000`
- **Used by**: `test-mta-hooks.sh`, `test-user-sync.sh`
- **Description**: Host URL of the SpacetimeDB instance under test.

### `DATABASE_NAME`
- **Default**: `kommunikation`
- **Used by**: `test-mta-hooks.sh`, `test-user-sync.sh`
- **Description**: Name of the SpacetimeDB database module being tested.

### `SPACETIME_WEBHOOK_TOKEN`
- **Default**: _(none)_
- **Used by**: Django backend settings (`solawispielplatz`)
- **Description**: Bearer token sent by Django signal handlers to the `/route/user-sync` endpoint for account synchronization.

---

## Deployment Profiles

### Local Development (`.env/.env.example`)
```ini
SPACETIMEDB_URI=http://localhost:3000
SPACETIMEDB_MODULE_NAME=kommunikation
SPACETIMEDB_DATABASE_NAME=kommunikation
SPACETIMEDB_TOKEN=

DJANGO_BASE_URL=http://127.0.0.1:8000
OIDC_ISSUER_URL=http://127.0.0.1:8000/o
OIDC_CLIENT_ID=admin-app
ADMIN_REDIRECT_URI=http://127.0.0.1:8080/callback
OAUTH_SCOPES=openid profile email

SMTP_HOST=localhost
SMTP_PORT=1025
SMTP_USE_TLS=false
SMTP_ACCEPT_INVALID_CERTS=false
SMTP_ACCEPT_INVALID_HOSTNAMES=false
MAIL_MESSAGE_ID_DOMAIN=localhost
MAIL_UNSUBSCRIBE_BASE_URL=http://localhost:3000/v1/database/kommunikation/route/mailing-list/unsubscribe
OTLP_ENDPOINT=http://localhost:4317

STALWART_JMAP_URL=http://localhost:8093/jmap
STALWART_ADMIN_TOKEN=

RUST_LOG=info
```

### Production Deployment (`.env/.env.production.example`)
```ini
SPACETIMEDB_URI=https://spacetimedb.your-domain.com
SPACETIMEDB_MODULE_NAME=kommunikation
SPACETIMEDB_DATABASE_NAME=kommunikation
SPACETIMEDB_TOKEN=your-sender-identity-token-here

DJANGO_BASE_URL=https://auth.your-domain.com
OIDC_ISSUER_URL=https://auth.your-domain.com/o
OIDC_CLIENT_ID=kommunikationszentrum-prod
ADMIN_REDIRECT_URI=https://admin.your-domain.com/callback
OAUTH_SCOPES=openid profile email

SMTP_HOST=mail-relay.your-domain.com
SMTP_PORT=465
SMTP_USE_TLS=true
SMTP_ACCEPT_INVALID_CERTS=false
SMTP_ACCEPT_INVALID_HOSTNAMES=false
MAIL_MESSAGE_ID_DOMAIN=your-domain.com
MAIL_UNSUBSCRIBE_BASE_URL=https://spacetimedb.your-domain.com/v1/database/kommunikation/route/mailing-list/unsubscribe
OTLP_ENDPOINT=http://alloy.internal:4317

STALWART_JMAP_URL=https://mail.your-domain.com:8093/jmap
STALWART_ADMIN_TOKEN=your-stalwart-admin-api-token-here

RUST_LOG=warn
```

---

## Configuration Validation

### Required Variables Checklist
- **Admin UI**:
  - `SPACETIMEDB_URI`
  - `DJANGO_BASE_URL`
  - `OIDC_CLIENT_ID`
  - `ADMIN_REDIRECT_URI`
- **Sender Daemon**:
  - `SPACETIMEDB_URI`
  - `SPACETIMEDB_DATABASE_NAME`
  - `SMTP_HOST` (mandatory; daemon exits if not set)
  - `SPACETIMEDB_TOKEN` (required for production so the admin identity is retained)

### Security Checklist
- [ ] No `.env` files containing secrets or production tokens are committed to git.
- [ ] `SPACETIMEDB_TOKEN` and `STALWART_ADMIN_TOKEN` are kept confidential.
- [ ] Production URLs strictly use `https://`.
- [ ] `SMTP_ACCEPT_INVALID_CERTS` and `SMTP_ACCEPT_INVALID_HOSTNAMES` are set to `false` in production.
- [ ] `RUST_LOG` is set to `warn` or `info` in production.
- [ ] `ADMIN_REDIRECT_URI` is registered and authorized in Django OAuth settings.