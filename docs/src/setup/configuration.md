# Configuration

The Kommunikationszentrum follows the [12-factor app](https://12factor.net/) methodology, using environment variables for configuration. This approach provides flexibility for different deployment environments while maintaining security best practices.

## Configuration Overview

All components support environment-based configuration with sensible defaults for development. Configuration is loaded in the following order:

1. Environment variables
2. `.env` file (if present)
3. Default values

## Environment Variables

### Core Database Configuration

| Variable | Default | Used by | Description |
|----------|---------|---------|-------------|
| `SPACETIMEDB_URI` | `http://localhost:3000` | admin, sender | SpacetimeDB server endpoint |
| `SPACETIMEDB_MODULE_NAME` | `kommunikation` | admin | SpacetimeDB module name |
| `SPACETIMEDB_DATABASE_NAME` | `kommunikation` | sender | SpacetimeDB module/database name |
| `SPACETIMEDB_TOKEN` | _(none)_ | sender | Authentication token for sender's admin identity |

### OAuth / Authentication Configuration

| Variable | Default | Used by | Description |
|----------|---------|---------|-------------|
| `DJANGO_BASE_URL` | `http://127.0.0.1:8000` | admin, server | Django OAuth provider base URL |
| `OIDC_ISSUER_URL` | `http://127.0.0.1:8000/o` | admin | OAuth issuer discovery URL |
| `OIDC_CLIENT_ID` | `admin-app` | admin | OAuth client identifier |
| `ADMIN_REDIRECT_URI` | `http://127.0.0.1:8080/callback` | admin | OAuth callback URL for admin UI |
| `OAUTH_SCOPES` | `openid profile email` | admin | Requested OAuth scopes |

### Sender Daemon Configuration

| Variable | Default | Used by | Description |
|----------|---------|---------|-------------|
| `SMTP_HOST` | _(none — required)_ | sender | Outbound SMTP relay hostname or IP |
| `SMTP_PORT` | `465` | sender | Outbound SMTP relay port |
| `SMTP_USE_TLS` | `true` | sender | Enable TLS for SMTP connection |
| `SMTP_ACCEPT_INVALID_CERTS` | `false` | sender | Accept self-signed certificates (dev only) |
| `SMTP_ACCEPT_INVALID_HOSTNAMES` | `false` | sender | Accept mismatched hostnames (dev only) |
| `MAIL_MESSAGE_ID_DOMAIN` | host of `SPACETIMEDB_URI` / `solawis.de` | sender | Domain for generated Message-ID headers |
| `MAIL_UNSUBSCRIBE_BASE_URL` | `<SPACETIMEDB_URI>/.../unsubscribe` | sender | Base URL for one-click unsubscribe links |
| `OTLP_ENDPOINT` | `http://localhost:4317` | sender | OpenTelemetry collector endpoint |

> [!NOTE]
> Outbound SMTP authentication is per-category via app passwords stored in SpacetimeDB (`CategoryAppPassword` table), not global environment variables.

### Stalwart Mailserver Fallback Configuration

| Variable | Default | Used by | Description |
|----------|---------|---------|-------------|
| `STALWART_JMAP_URL` | _(none)_ | server | Stalwart JMAP URL (fallback if not configured in DB) |
| `STALWART_ADMIN_TOKEN` | _(none)_ | server | Stalwart Admin API key (fallback if not configured in DB) |

### Logging Configuration

| Variable | Default | Used by | Description |
|----------|---------|---------|-------------|
| `RUST_LOG` | `info` | all | Tracing/logging filter directive (e.g. `info`, `sender=trace`) |

## Configuration Files

The `.env/` directory provides pre-configured template files for different components and environments:

- `.env/.env.example`: Complete environment template for local development.
- `.env/.env.production.example`: Full production configuration template.
- `.env/.env.sender.example` / `.env/.env.webhook-proxy.example`: Standalone environment file for running the sender daemon.
- `.env/.env.stalwart-jmap.example`: Environment file for Stalwart JMAP credentials.
- `.env/.env.oidc`: Environment variables for OIDC authentication.

### Development Configuration

For local development, copy the example configuration:

```bash
cp .env/.env.example .env
```

The `.env/.env.example` file contains development-appropriate defaults:

```ini
# SpacetimeDB Configuration (Admin & Sender)
SPACETIMEDB_URI=http://localhost:3000
SPACETIMEDB_MODULE_NAME=kommunikation
SPACETIMEDB_DATABASE_NAME=kommunikation
SPACETIMEDB_TOKEN=

# Django / OAuth Configuration (Admin & Server)
DJANGO_BASE_URL=http://127.0.0.1:8000
OIDC_ISSUER_URL=http://127.0.0.1:8000/o
OIDC_CLIENT_ID=admin-app

# Admin Web App Configuration (Admin)
ADMIN_REDIRECT_URI=http://127.0.0.1:8080/callback
OAUTH_SCOPES=openid profile email

# Sender Daemon Configuration (Sender)
SMTP_HOST=localhost
SMTP_PORT=1025
SMTP_USE_TLS=false
SMTP_ACCEPT_INVALID_CERTS=false
SMTP_ACCEPT_INVALID_HOSTNAMES=false
MAIL_MESSAGE_ID_DOMAIN=localhost
MAIL_UNSUBSCRIBE_BASE_URL=http://localhost:3000/v1/database/kommunikation/route/mailing-list/unsubscribe
OTLP_ENDPOINT=http://localhost:4317

# Stalwart JMAP Configuration (Server fallback)
STALWART_JMAP_URL=http://localhost:8093/jmap
STALWART_ADMIN_TOKEN=

# Logging & Tracing
RUST_LOG=info
```

### Production Configuration

For production deployments, see `.env/.env.production.example` for guidance:

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

## Component-Specific Configuration

### Admin Web Interface

The admin interface loads configuration in the browser:

```rust
// Loads configuration from environment variables set at build time
let config = AdminConfig::load();
```

For WASM targets, environment variables must be set during the build process (`dx serve` or `dx build`).

### Sender Daemon

The sender daemon loads its configuration using `SenderConfig::from_env()` on startup:

```bash
# Sourcing the sender environment before running
sh -lc '. .env/.env.sender && cargo run --package sender'
```

### SpacetimeDB Server Module

Since SpacetimeDB modules run in a sandboxed WASM environment, compile-time defaults can be configured via `option_env!`:

```rust
const DJANGO_OAUTH_BASE_URL: &str = match option_env!("DJANGO_BASE_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:8000",
};
```

Runtime settings for Stalwart mailserver are managed via the `set_stalwart_config` reducer (persisted in SpacetimeDB) or fallback environment variables `STALWART_JMAP_URL` and `STALWART_ADMIN_TOKEN`.

## Security Considerations

### Sensitive Information

- Never commit `.env` files containing sensitive tokens, passwords, or production secrets to version control.
- In production, set environment variables directly via process managers (e.g. systemd `EnvironmentFile` or container orchestration).
- Regularly rotate OAuth client secrets and Stalwart admin API keys.

### HTTPS in Production

For production deployments:
- Use HTTPS URLs for all external endpoints (`SPACETIMEDB_URI`, `DJANGO_BASE_URL`, `OIDC_ISSUER_URL`, `MAIL_UNSUBSCRIBE_BASE_URL`).
- Update OAuth redirect URIs to use HTTPS.
- Set `SMTP_USE_TLS=true`, `SMTP_ACCEPT_INVALID_CERTS=false`, and `SMTP_ACCEPT_INVALID_HOSTNAMES=false`.

## Troubleshooting Configuration

### Verification

Check configuration loading by examining startup logs:

```bash
# View module logs at startup
spacetime logs kommunikation --follow

# View sender logs with debug level
RUST_LOG=sender=debug cargo run --package sender
```

### Common Issues

**Connection failures**: Verify `SPACETIMEDB_URI` is correct and SpacetimeDB is running.

**OAuth errors**: Check `DJANGO_BASE_URL` and `OIDC_CLIENT_ID` match your Django OAuth provider configuration.

**SMTP relay errors**: Ensure `SMTP_HOST` is set, and verify TLS settings match relay requirements.
