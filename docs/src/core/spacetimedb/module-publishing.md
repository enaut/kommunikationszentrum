# Module Publishing

How to build, publish, and manage the SpacetimeDB module.

## Prerequisites

- SpacetimeDB CLI installed
- Rust toolchain with `wasm32-unknown-unknown` target
- SpacetimeDB running on port 3000
- Environment variables set:
  - `STALWART_ADMIN_TOKEN`
  - `STALWART_JMAP_URL`

Build and publish require these env vars, so they cannot be run by AI agents.

## Build and Publish

```bash
spacetime build -p server kommunikation
spacetime publish -p server kommunikation
```

Use `-c` with publish to clear the database when the schema changes (deletes all data):

```bash
spacetime publish -p server kommunikation -c
```

## Client Bindings

Regenerate bindings after schema or reducer changes:

```bash
spacetimedb-cli generate --lang dioxus -p server -o admin/src/module_bindings/
spacetimedb-cli generate --lang rust -p server -o sender/src/module_bindings/
```

This uses a custom `spacetimedb-cli` and cannot be run by AI agents.

## SpacetimeDB Server

```bash
spacetime start   # localhost:3000
```

## Status and Debugging

```bash
spacetime describe kommunikation
spacetime list
spacetime logs kommunikation
spacetime logs kommunikation --follow
```

```bash
spacetime sql kommunikation "SELECT COUNT(*) FROM account"
spacetime sql kommunikation "SELECT * FROM message_categories LIMIT 10"
spacetime sql kommunikation "SELECT * FROM mta_connection_log ORDER BY timestamp DESC LIMIT 5"
```

## Development Workflow

1. Edit schema or reducers in `server/`
2. Build: `spacetime build -p server kommunikation`
3. Publish: `spacetime publish -p server kommunikation` (add `-c` for schema resets)
4. Regenerate client bindings (see above)
5. Debug with `spacetime logs kommunikation`
