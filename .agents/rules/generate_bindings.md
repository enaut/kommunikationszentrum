---
name: generate-bindings
description: How to regenerate SpacetimeDB bindings for admin and sender
trigger: always_on
---

# Regenerating SpacetimeDB Bindings

When regenerating SpacetimeDB module bindings for the admin app or sender, you MUST use the local custom `spacetimedb-cli` binary possibly located at:
`/home/<user>/Projekte/Source/SpacetimeDB/target/release/spacetimedb-cli`

Do NOT use the system `spacetimedb-cli` or `spacetime generate`.

### Commands

For the Dioxus Admin application:
```bash
/home/<user>/Projekte/Source/SpacetimeDB/target/release/spacetimedb-cli generate --lang dioxus -p server -o admin/src/module_bindings/
```

For the Rust Sender service:
```bash
/home/<user>/Projekte/Source/SpacetimeDB/target/release/spacetimedb-cli generate --lang rust -p server -o sender/src/module_bindings/
```

When prompted by the CLI regarding stale files to delete, confirm deletion.
