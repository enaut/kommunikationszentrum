# Configuration

All sender daemon configuration is loaded at startup from **environment variables**. There are no runtime config files.

## Configuration Flow Map

```d2
{{#include sender-config-map.d2}}
```

## Environment Variables Reference

| Variable | Default | Description |
|---|---|---|
| `SPACETIMEDB_URI` | `http://127.0.0.1:3000` | WebSocket/HTTP URL of the SpacetimeDB instance |
| `SPACETIMEDB_DATABASE_NAME` | `kommunikation` | Database / module name |
| `SPACETIMEDB_TOKEN` | _(none)_ | Authentication token for the sender's admin identity |
| `SMTP_HOST` | `mail-eu.smtp2go.com` | Hostname of the outbound SMTP relay |
| `SMTP_PORT` | `8465` | SMTP port (e.g. 587 for STARTTLS, 465/8465 for TLS, 25 for local relay) |
| `SMTP_USERNAME` | _(none)_ | Optional SMTP AUTH username |
| `SMTP_PASSWORD` | _(none)_ | Optional SMTP AUTH password |
| `SMTP_USE_TLS` | `true` | Enable TLS (`true` for production relays, `false` for local debug relays) |
| `SMTP_ACCEPT_INVALID_CERTS` | `false` | Accept expired or self-signed SMTP server certificates when TLS is enabled |
| `SMTP_ACCEPT_INVALID_HOSTNAMES` | `false` | Accept mismatched certificate hostnames for SMTP TLS connections |
| `MAIL_MESSAGE_ID_DOMAIN` | derived from `SPACETIMEDB_URI` | Domain used in generated `Message-ID` headers |
| `MAIL_UNSUBSCRIBE_BASE_URL` | `<SPACETIMEDB_URI>/.../unsubscribe` | Endpoint for HTTPS one-click unsubscribe links |
| `OTLP_ENDPOINT` | `http://localhost:4317` | OpenTelemetry gRPC collector endpoint (Alloy / Jaeger) |
| `RUST_LOG` | `sender=info` | Tracing log filter directive |

## Environment Profiles

### Local Development (`.env`)

```dotenv
SPACETIMEDB_URI=http://localhost:3000
SPACETIMEDB_DATABASE_NAME=kommunikationszentrum
SPACETIMEDB_TOKEN=<sender-auth-token>

SMTP_HOST=localhost
SMTP_PORT=1025
SMTP_USE_TLS=false
SMTP_ACCEPT_INVALID_CERTS=false
SMTP_ACCEPT_INVALID_HOSTNAMES=false

MAIL_MESSAGE_ID_DOMAIN=localhost
MAIL_UNSUBSCRIBE_BASE_URL=http://localhost:3000/v1/database/kommunikationszentrum/route/mailing-list/unsubscribe

OTLP_ENDPOINT=http://localhost:4317
RUST_LOG=sender=debug
```

### Production Deployment

```dotenv
SPACETIMEDB_URI=https://spacetimedb.example.org
SPACETIMEDB_DATABASE_NAME=kommunikationszentrum
SPACETIMEDB_TOKEN=<production-secret-token>

SMTP_HOST=mail-eu.smtp2go.com
SMTP_PORT=8465
SMTP_USERNAME=relay_user
SMTP_PASSWORD=relay_secret_password
SMTP_USE_TLS=true
SMTP_ACCEPT_INVALID_CERTS=false
SMTP_ACCEPT_INVALID_HOSTNAMES=false

MAIL_MESSAGE_ID_DOMAIN=solawis.de
MAIL_UNSUBSCRIBE_BASE_URL=https://spacetimedb.example.org/v1/database/kommunikationszentrum/route/mailing-list/unsubscribe

OTLP_ENDPOINT=http://alloy.internal:4317
RUST_LOG=sender=info
```
