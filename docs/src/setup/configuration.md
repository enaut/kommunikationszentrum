# Configuration

Kommunikationszentrum employs a two-tier configuration architecture designed to optimize security, branding flexibility, and deployment ergonomics across its different components:

1. **Compile-Time Configuration Profiles (`profiles/*.toml`)**: Used by the **Admin Web Interface** (`admin`) and the **SpacetimeDB Server Module** (`server`). Configuration settings—including backend URLs, OpenID Connect parameters, visual branding, and localized copy—are compiled directly into WebAssembly binaries at build time.
2. **Runtime Environment Variables (`.env`)**: Used by the background **Sender Daemon** (`sender`) and operational logging/tracing. Follows the [12-factor app](https://12factor.net/) methodology.
3. **Database-Backed Runtime Settings**: Outbound topic SMTP credentials (`TopicAppPassword`) and Stalwart mail server integration parameters are stored securely inside SpacetimeDB tables, managed dynamically via reducers or the Admin UI.

---

## 1. Build-Time Configuration Profiles (`admin` & `server`)

Both the admin frontend and server module resolve configuration from a profile TOML file during build time.

### Quick Setup

1. Copy the tracked template profile to create an organization profile:
   ```bash
   cp profiles/example.toml profiles/my-org.toml
   ```
2. Adjust your endpoints, OIDC provider configuration, branding, and color palette in `profiles/my-org.toml`.
3. Build or serve the components by setting `APP_PROFILE`:

   ```bash
   # Build the WebAssembly admin interface
   APP_PROFILE=my-org dx build -p admin

   # Run the admin interface with live reloading
   APP_PROFILE=my-org dx serve -p admin

   # Publish the server module to SpacetimeDB
   APP_PROFILE=my-org spacetime publish -p server kommunikation
   ```

> [!TIP]
> Custom profile files (`profiles/*.toml` except `example.toml`) are gitignored automatically to keep organization domains, OAuth credentials, and theme tokens private.

> [!IMPORTANT]
> For the complete schema, list of all supported fields, color tokens, and localized Fluent brand keys, see the [Configuration Profiles Reference](../reference/configuration.md).

---

## 2. Runtime Environment Variables (`sender`)

The `sender` daemon is a native Rust background service responsible for SMTP delivery, retry queues, and OpenTelemetry instrumentation. It reads configuration from environment variables on startup.

### Sender Configuration Options

| Variable | Default | Description |
|---|---|---|
| `SPACETIMEDB_URI` | `http://127.0.0.1:3000` | HTTP / WebSocket URL of the SpacetimeDB instance |
| `SPACETIMEDB_DATABASE_NAME` | `kommunikation` | Database / module name to connect to |
| `SPACETIMEDB_TOKEN` | _(none)_ | Admin identity authentication token for the sender daemon |
| `SMTP_HOST` | _(required)_ | Outbound SMTP relay host or IP |
| `SMTP_PORT` | `465` | Outbound SMTP relay port |
| `SMTP_USE_TLS` | `true` | Enable TLS for SMTP connection |
| `SMTP_ACCEPT_INVALID_CERTS` | `false` | Accept self-signed certificates (dev only) |
| `SMTP_ACCEPT_INVALID_HOSTNAMES` | `false` | Accept mismatched hostnames (dev only) |
| `SMTP_SYSTEM_USERNAME` | _(none)_ | SMTP username for transactional system emails (e.g. verification tokens) |
| `SMTP_SYSTEM_PASSWORD` | _(none)_ | SMTP password for transactional system emails |
| `MAIL_MESSAGE_ID_DOMAIN` | Host of `SPACETIMEDB_URI` / `localhost` | Domain for generated `Message-ID` headers |
| `MAIL_UNSUBSCRIBE_BASE_URL` | `<SPACETIMEDB_URI>/.../unsubscribe` | RFC 8058 one-click HTTPS unsubscribe endpoint |
| `OTLP_ENDPOINT` | `http://localhost:4317` | OpenTelemetry collector endpoint |
| `RUST_LOG` | `info` | Tracing / logging filter directive (e.g. `sender=debug,info`) |

> [!NOTE]
> Outbound SMTP authentication for topic mailing lists is per-topic via app passwords stored in SpacetimeDB (`TopicAppPassword` table), not global environment variables. In contrast, transactional system emails authenticate using `SMTP_SYSTEM_USERNAME` and `SMTP_SYSTEM_PASSWORD`.

### Example Sender `.env` File

For local development:

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

Run the sender with the environment file loaded:

```bash
sh -lc '. .env/.env.sender && cargo run --package sender'
```

> [!TIP]
> For a full list and descriptions of all runtime environment variables, see the [Environment Variables Reference](../reference/environment-variables.md).

---

## 3. Database-Backed Settings (Stalwart MTA)

Runtime settings for the Stalwart mail server (JMAP URL and Admin API token) can be configured dynamically after publishing via the `set_stalwart_config` reducer or through the Admin UI.

---

## Security Considerations

- Never commit custom `.env` files or custom `profiles/*.toml` containing sensitive production secrets to public version control.
- In production, set runtime environment variables for the sender daemon directly via systemd unit files (`EnvironmentFile`) or container orchestration.
- Use HTTPS and TLS for all production endpoints (`spacetimedb.uri`, `oauth.issuer_url`, `SMTP_USE_TLS=true`).

---

## Troubleshooting

- **Admin or Server using wrong endpoints**: Verify `APP_PROFILE` is set when running `dx build` or `spacetime publish`. When unspecified, both fall back to `profiles/example.toml`.
- **Sender connection failures**: Verify `SPACETIMEDB_URI` matches the running SpacetimeDB instance and `SPACETIMEDB_TOKEN` is authorized.
- **System email sending failures**: Ensure `SMTP_SYSTEM_USERNAME` and `SMTP_SYSTEM_PASSWORD` are configured if self-service verification emails are enabled.
