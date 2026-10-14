# SpacetimeDB Server Setup

The **SpacetimeDB Server Module** (`server`) forms the central database and business logic engine of Kommunikationszentrum. It executes inside SpacetimeDB as a sandboxed WebAssembly module, providing real-time client tables, views, reducers, and HTTP endpoints.

---

## Prerequisites

- **Rust toolchain** with `wasm32-unknown-unknown` target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- **SpacetimeDB CLI** (`spacetime`):
  ```bash
  cargo install spacetime
  ```
- **Running SpacetimeDB Instance**:
  ```bash
  spacetime start
  ```

---

## Configuration & Profiles

The server module resolves configuration at compile time via `server/build.rs`. The trusted OIDC issuer, frontend base URL (for verification emails), and localized instance brand names are extracted directly from a configuration profile in `profiles/`:

- `[oauth].issuer_url`: Used for strict `iss` validation during `register_self` and for deriving synced account identities.
- `[server].frontend_base_url`: Used to construct email verification and unsubscribe links.
- `[brand.i18n.de].brand_name` and `[brand.i18n.en].brand_name`: Used in transactional system emails (rejection notices and verification emails).

> [!TIP]
> For the complete schema and options, see the [Configuration Profiles Reference](../reference/configuration.md).

---

## Building and Publishing

When publishing the module to a local or remote SpacetimeDB instance, specify the target profile using `APP_PROFILE`:

```bash
# Publish module to local SpacetimeDB instance
APP_PROFILE=my-org spacetime publish -p server kommunikation

# If schema changes require clearing the database tables:
APP_PROFILE=my-org spacetime publish -p server kommunikation -c
```

If `APP_PROFILE` is omitted, the build defaults to `profiles/example.toml`.

---

## Verifying and Testing

You can run the server unit test suite against any profile:

```bash
# Run unit tests with the default example profile
cargo test -p spacetime-module

# Run unit tests with a specific profile
APP_PROFILE=my-org cargo test -p spacetime-module
```

Inspect module logs and status:

```bash
# Follow server module logs
spacetime logs kommunikation --follow

# Describe module tables and reducers
spacetime describe kommunikation
```
