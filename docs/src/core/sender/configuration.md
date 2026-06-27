# Configuration

All sender configuration is loaded at startup from **environment variables**. There are no
config files — copy `.env.example` and set the relevant `SMTP_*` / `SPACETIMEDB_*` variables.

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `SPACETIMEDB_URI` | `http://127.0.0.1:3000` | WebSocket/HTTP base URL of SpacetimeDB |
| `SPACETIMEDB_DATABASE_NAME` | `kommunikation` | SpacetimeDB module/database name |
| `SPACETIMEDB_TOKEN` | _(none)_ | Auth token for the sender's SpacetimeDB identity. If omitted, an anonymous identity is used (works only if the module allows it). |
| `SMTP_HOST` | `mail-eu.smtp2go.com` | Hostname of the outbound SMTP relay |
| `SMTP_PORT` | `8465` | SMTP relay port |
| `SMTP_USERNAME` | _(none)_ | SMTP AUTH username (optional) |
| `SMTP_PASSWORD` | _(none)_ | SMTP AUTH password (optional) |
| `SMTP_USE_TLS` | `true` | Whether to use TLS when connecting to the relay (`true` = SMTPS/STARTTLS; `false` = plaintext, for local relays only) |
| `SENDER_POLL_INTERVAL_MS` | `5000` | Fallback poll interval in ms. The daemon is primarily event-driven; this is unused in practice but kept for future fallback use. |
| `MAIL_MESSAGE_ID_DOMAIN` | derived from `SPACETIMEDB_URI` host | Domain used in generated `Message-ID` headers (`<seed@domain>`) |
| `MAIL_UNSUBSCRIBE_BASE_URL` | `<SPACETIMEDB_URI>/v1/database/<NAME>/route/mailing-list/unsubscribe` | Base URL embedded in `List-Unsubscribe` headers |
| `OTLP_ENDPOINT` | `http://localhost:4317` | OTLP gRPC endpoint for traces and logs (e.g. Grafana Alloy) |
| `RUST_LOG` | `sender=info` | Log filter directive, passed to `tracing-subscriber`'s `EnvFilter` |

## Configuration Source

The `SenderConfig` struct in [config.rs](file:///home/dietrich/Projekte/Source/kommunikationszentrum/sender/src/config.rs)
reads all variables at startup via `std::env::var`. Missing optional variables fall back to
the defaults shown above. There is no hot-reload — changes require a restart.

```rust
pub struct SenderConfig {
    pub spacetimedb_uri: String,
    pub spacetimedb_database_name: String,
    pub spacetimedb_token: Option<String>,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_use_tls: bool,
    pub poll_interval: Duration,
    pub message_id_domain: String,
    pub unsubscribe_base_url: String,
    pub otlp_endpoint: String,
}
```

## Example `.env` (Development)

```dotenv
SPACETIMEDB_URI=http://localhost:3000
SPACETIMEDB_DATABASE_NAME=kommunikationszentrum
SPACETIMEDB_TOKEN=<your-sender-token>

SMTP_HOST=localhost
SMTP_PORT=25
SMTP_USE_TLS=false

MAIL_MESSAGE_ID_DOMAIN=dev.example.org
MAIL_UNSUBSCRIBE_BASE_URL=http://localhost:3000/v1/database/kommunikationszentrum/route/mailing-list/unsubscribe

OTLP_ENDPOINT=http://localhost:4317
RUST_LOG=sender=debug
```

## Example `.env` (Production)

```dotenv
SPACETIMEDB_URI=https://spacetimedb.example.org
SPACETIMEDB_DATABASE_NAME=kommunikationszentrum
SPACETIMEDB_TOKEN=<secret-token>

SMTP_HOST=mail-eu.smtp2go.com
SMTP_PORT=8465
SMTP_USERNAME=myusername
SMTP_PASSWORD=mypassword
SMTP_USE_TLS=true

MAIL_MESSAGE_ID_DOMAIN=example.org
MAIL_UNSUBSCRIBE_BASE_URL=https://spacetimedb.example.org/v1/database/kommunikationszentrum/route/mailing-list/unsubscribe

OTLP_ENDPOINT=http://alloy.internal:4317
RUST_LOG=sender=info
```

## SpacetimeDB Identity & Token

The sender daemon connects to SpacetimeDB with a persistent identity so the server module can
identify it as the claim owner for `MailIngress` and `MailDelivery` records.

1. On first startup without a token, SpacetimeDB issues an anonymous identity and token.
   The daemon logs its identity at startup:
   ```
   INFO sender connected as Some(Identity { ... })
   ```
2. Save the issued token to `SPACETIMEDB_TOKEN` for subsequent restarts so the same identity
   is reused.
3. The sender's identity **must be added to `admin_identities`** on the server module, otherwise
   claim reducer calls will be rejected.

```bash
# Register the sender identity as admin (run once after first startup)
spacetime call kommunikationszentrum register_admin_identity "<sender-identity-hex>"
```
