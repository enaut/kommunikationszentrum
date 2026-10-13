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
| `SMTP_HOST` | _(none — required)_ | Hostname of the outbound SMTP relay (required) |
| `SMTP_PORT` | `465` | SMTP port (e.g. 587 for STARTTLS, 465 for TLS, 25 for local relay) |
| `SMTP_USE_TLS` | `true` | Enable TLS (`true` for production relays, `false` for local debug relays) |
| `SMTP_ACCEPT_INVALID_CERTS` | `false` | Accept expired or self-signed SMTP server certificates when TLS is enabled |
| `SMTP_ACCEPT_INVALID_HOSTNAMES` | `false` | Accept mismatched certificate hostnames for SMTP TLS connections |
| `SMTP_SYSTEM_USERNAME` | _(none)_ | Username for authenticating transactional system emails (e.g. email verification tokens). Also used as the sender address in `From` headers when containing `@` (otherwise `no-reply@{MAIL_MESSAGE_ID_DOMAIN}`). Required if system email dispatch is enabled. |
| `SMTP_SYSTEM_PASSWORD` | _(none)_ | Password or app password for the system user on the outbound SMTP relay. Required if system email dispatch is enabled. |
| `MAIL_MESSAGE_ID_DOMAIN` | derived from `SPACETIMEDB_URI` / `solawis.de` | Domain used in generated `Message-ID` headers |
| `MAIL_UNSUBSCRIBE_BASE_URL` | `<SPACETIMEDB_URI>/.../unsubscribe` | Endpoint for RFC 8058 HTTPS one-click unsubscribe links |
| `OTLP_ENDPOINT` | `http://localhost:4317` | OpenTelemetry gRPC collector endpoint (Alloy / Jaeger) |
| `RUST_LOG` | `sender=info` | Tracing log filter directive |

> [!NOTE]
> **SMTP TLS Modes**: When `SMTP_USE_TLS` is `true`, connecting to port `465` automatically uses implicit TLS (`Tls::Wrapper`), while other ports (such as `587`) use explicit STARTTLS (`Tls::Required`). If `SMTP_ACCEPT_INVALID_CERTS` or `SMTP_ACCEPT_INVALID_HOSTNAMES` is enabled, the sender configures permissive TLS parameters via `builder_dangerous`.
>
> **SMTP Credentials**:
> - **Mailing List Deliveries**: Global `SMTP_USERNAME` and `SMTP_PASSWORD` environment variables are **not** used for topic deliveries. SMTP authentication credentials (username and app password) are configured per message topic and stored securely in SpacetimeDB (`TopicAppPassword` table), managed through the Admin Web UI.
> - **Transactional System Emails**: Dedicated system user credentials configured via `SMTP_SYSTEM_USERNAME` and `SMTP_SYSTEM_PASSWORD` are used to authenticate system emails (such as self-service email verification tokens queued in `sender_system_mail_pending`). If either variable is omitted, the sender skips system mail dispatch and logs an error.

## Environment Profiles

### Local Development (`.env/.env.sender` or `.env/.env.webhook-proxy`)

```dotenv
SPACETIMEDB_URI=http://localhost:3000
SPACETIMEDB_DATABASE_NAME=kommunikation
SPACETIMEDB_TOKEN=

SMTP_HOST=localhost
SMTP_PORT=1025
SMTP_USE_TLS=false
SMTP_ACCEPT_INVALID_CERTS=false
SMTP_ACCEPT_INVALID_HOSTNAMES=false
SMTP_SYSTEM_USERNAME=system@localhost
SMTP_SYSTEM_PASSWORD=system-password

MAIL_MESSAGE_ID_DOMAIN=localhost
MAIL_UNSUBSCRIBE_BASE_URL=http://localhost:3000/v1/database/kommunikation/route/mailing-list/unsubscribe

OTLP_ENDPOINT=http://localhost:4317
RUST_LOG=sender=debug
```

### Production Deployment

```dotenv
SPACETIMEDB_URI=https://spacetimedb.example.org
SPACETIMEDB_DATABASE_NAME=kommunikation
SPACETIMEDB_TOKEN=<production-secret-token>

SMTP_HOST=mx.example.org
SMTP_PORT=465
SMTP_USE_TLS=true
SMTP_ACCEPT_INVALID_CERTS=false
SMTP_ACCEPT_INVALID_HOSTNAMES=false
SMTP_SYSTEM_USERNAME=system@example.org
SMTP_SYSTEM_PASSWORD=<production-system-password>

MAIL_MESSAGE_ID_DOMAIN=solawis.de
MAIL_UNSUBSCRIBE_BASE_URL=https://spacetimedb.example.org/v1/database/kommunikation/route/mailing-list/unsubscribe

OTLP_ENDPOINT=http://alloy.internal:4317
RUST_LOG=sender=info
```
