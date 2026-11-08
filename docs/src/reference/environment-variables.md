# Environment Variables Reference

The Admin frontend and SpacetimeDB module use build-time application profiles. The SpacetimeDB module also declares server-side environment values for authentication and JMAP delivery. There is no standalone sender process.

### Build-time profile selection

#### `APP_PROFILE`

- **Default**: `example`
- **Used by**: `admin/build.rs`, `server/build.rs`, and `scripts/publish_spacetime.py`
- **Description**: Selects `profiles/<name>/profile.toml` for build-time configuration.

#### `APP_CONFIG_PATH`

- **Default**: none
- **Used by**: `admin/build.rs`, `server/build.rs`
- **Description**: Overrides profile selection with a TOML file or profile directory path.

### SpacetimeDB module environment

The module declares these values with `#[spacetimedb::env]`. Secret values are read server-side and are not exposed through table subscriptions. See [Outbound Delivery](../core/outbound-delivery.md) for dispatch and queue behavior.

#### `OIDC_ISSUER_URL`

- **Used by**: SpacetimeDB module
- **Description**: Issuer used to validate registration JWTs and derive synchronized user identities. The profile-aware publish script derives it from the selected profile.

#### `SYSTEM_JMAP_USERNAME`

- **Used by**: SpacetimeDB module
- **Description**: JMAP account username for transactional system messages.

#### `SYSTEM_JMAP_PASSWORD`

- **Used by**: SpacetimeDB module
- **Description**: App password for the system JMAP account. Keep it in deployment environment configuration, not in source control or delivery rows.

#### `JMAP_DISPATCH_ENABLED`

- **Default**: `false`
- **Used by**: SpacetimeDB module
- **Description**: Set to `true` to enable scheduled JMAP delivery. Check outstanding legacy queue records and validate the target Stalwart account before enabling dispatch.

#### `MESSAGE_ID_DOMAIN`

- **Used by**: SpacetimeDB module
- **Description**: Domain used when deriving message IDs and system sender addresses.

#### `MAIL_UNSUBSCRIBE_BASE_URL`

- **Used by**: SpacetimeDB module
- **Description**: Base URL used to construct mailing-list `List-Unsubscribe` headers.

### Stalwart settings

The Stalwart JMAP URL and admin API token are stored in the `stalwart_config` table and managed through the Admin UI or `set_stalwart_config`. The module also supports `STALWART_JMAP_URL` and `STALWART_ADMIN_TOKEN` as fallbacks when database settings are empty.

### Observability and integrations

#### `RUST_LOG`

- **Default**: `info`
- **Used by**: Rust build and operational tools
- **Description**: Tracing filter directive, for example `info` or `spacetime_module=debug`.

#### `SPACETIME_WEBHOOK_TOKEN`

- **Used by**: Django `solawispielplatz`
- **Description**: Bearer token sent to the `/route/user-sync` endpoint.

#### `OIDC_TIMEOUT`

- **Default**: `180`
- **Used by**: OIDC helper script
- **Description**: Timeout in seconds for the local browser callback.
