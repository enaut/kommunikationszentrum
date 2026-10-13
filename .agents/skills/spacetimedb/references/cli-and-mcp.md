# CLI Workflows & Native MCP in SpacetimeDB v2.11

The SpacetimeDB CLI manages database modules, schema migrations, client binding generation, and AI agent integration via Model Context Protocol (MCP).

---

## 1. Core CLI Commands

### Project Initialization
```bash
# Initialize a new Rust server module
spacetime init --template basic-rs my-project

# Initialize with a frontend client
spacetime init --template react-ts my-fullstack-app
```

### Development & Hot-Reloading
```bash
# Runs local server, watches for code changes, recompiles, and syncs bindings
spacetime dev
```

### Building & Publishing Modules
```bash
# Compile to WebAssembly without publishing
spacetime build

# Publish or update database module
spacetime publish my-database

# Publish with breaking client changes
spacetime publish my-database --break-clients

# Reset state completely (delete all data and republish)
spacetime publish my-database --delete-data
```

### Inspecting Schemas & Logs
```bash
# v2.11 Human-readable schema inspection (tables, reducers, views, types, constraints)
spacetime describe my-database

# Live tail of module debug logs
spacetime logs my-database -f

# Run interactive SQL query
spacetime sql my-database "SELECT * FROM player WHERE score > 100"

# Direct invocation of reducer or procedure from shell
spacetime call my-database add_person '["Alice", 30]'
```

### Generating Client SDK Bindings
```bash
# For Rust client
spacetime generate --lang rust -p server -o client/src/module_bindings/

# For Dioxus client
spacetime generate --lang dioxus -p server -o admin/src/module_bindings/

# For TypeScript / React
spacetime generate --lang typescript -p server -o frontend/src/module_bindings/
```

---

## 2. Schema Migrations (`publish`)

When you run `spacetime publish <DATABASE>`, the engine analyzes the delta between your existing database state and the new module schema:

### Automatic / Safe Migrations:
* Adding new tables.
* Adding new columns with default values or at the end of the schema.
* Adding BTree or composite indexes.
* Adding or removing `#[auto_inc]`.
* Adding new reducers, procedures, or views.
* Changing a table from `private` to `public`.
* Removing unique constraints.

### Breaking Migrations (Requires `--break-clients`):
* Removing public tables or changing public tables to private.
* Removing existing columns or reordering columns.
* Removing or modifying signatures of existing reducers or procedures.
* Modifying column data types.

---

## 3. Native Model Context Protocol (MCP) Server

Clockwork Labs embeds native MCP support directly into the core SpacetimeDB engine. Any AI coding assistant or agent can directly inspect schemas, list tables, run SQL, and invoke reducers/procedures without writing custom Node.js bridges.

### Local Stdio MCP Configuration

Add SpacetimeDB to your AI tool's `mcpServers` configuration (`claude_desktop_config.json`, Cursor, Antigravity, etc.):

```json
{
  "mcpServers": {
    "spacetimedb": {
      "command": "spacetime",
      "args": ["mcp"]
    }
  }
}
```

When running `spacetime mcp`:
* Target database settings are resolved from the current directory's `spacetime.json`.
* Exposes tools for:
  * `list_tables`: Discover all relational schemas and row counts.
  * `describe_table`: Detailed column definitions, keys, and indexes.
  * `execute_sql`: Read and verify live database state with standard SQL queries.
  * `call_reducer`: Test state transitions and trigger logic directly.
  * `call_procedure`: Test external HTTP integration and return values.

### Maincloud MCP Endpoint
For hosted deployments on Clockwork Labs Maincloud, connect directly using the native `/v1/mcp` endpoint with your Maincloud identity token.
