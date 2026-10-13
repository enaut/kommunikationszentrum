---
name: spacetimedb
description: Authoritative guide for building SpacetimeDB v2.11+ applications in Rust. Use when writing SpacetimeDB server modules, schemas, tables, event tables, reducers, procedures, HTTP handlers/routers, views, subscriptions, Rust client bindings, CLI workflows, or native MCP integration.
license: MIT
---

# SpacetimeDB v2.11 Rust Architecture

SpacetimeDB collapses the server application and relational database into a unified WebAssembly module running an in-memory transactional state engine.

## Core Architectural Mental Model

1. **State is Relational & In-Memory:** Tables hold relational data. In-memory queries and writes run in nanoseconds. Commits append directly to an append-only, durable commitlog.
2. **Reducers (Transactional Writes):** Reducers are the primary write entrypoint. Each reducer executes inside an isolated transaction. Returning `Ok(())` commits all mutations; returning `Err(...)` or panicking aborts and rolls back changes completely.
3. **Procedures (v2.0+ Side Effects & Return Values):** Procedures can return values to the caller and execute side-effecting operations (outbound HTTP requests via `ctx.http`, sleeping). Procedures **do not** automatically execute in a database transaction; transactions must be explicitly demarcated with `ctx.with_tx` or `ctx.try_with_tx`.
4. **HTTP Handlers & Routers (v2.0+ Webhooks):** Modules can expose HTTP endpoints using `#[spacetimedb::http::handler]` and map them using `#[spacetimedb::http::router]`.
5. **Views (v2.0+ Read-Only Slices):** Server-side query functions returning `Vec<T>` or `Option<T>`. Anonymous views (`&AnonymousViewContext`) are caller-agnostic and materialized across all subscriptions; sender-aware views (`&ViewContext`) filter by `ctx.sender()`.
6. **Event Tables (v2.0+ Transient Messaging):** Ephemeral tables marked `#[table(accessor = ..., event)]`. Rows exist only during transaction execution. On commit, rows are broadcast to subscribers and deleted immediately from the database state. Replaces deprecated 1.x reducer callbacks.
7. **Confirmed Reads:** Subscription updates and SQL query results are sent only after transactions are committed and durable.

---

## Decision Matrix: Function Types

| Need | Macro / Construct | Transaction Scope | Side Effects / Network | Return Type |
|---|---|---|---|---|
| Mutate database state | `#[spacetimedb::reducer]` | Automatic transaction (rolls back on Err/panic) | Forbidden (pure DB logic) | `()` or `Result<(), E>` (error only) |
| Call external HTTP API | `#[spacetimedb::procedure]` | Manual via `ctx.with_tx` / `ctx.try_with_tx` | Outbound HTTP allowed (`ctx.http`) | Any `SpacetimeType` |
| Inbound HTTP Webhook | `#[spacetimedb::http::handler]` | Manual via `ctx.with_tx` / `ctx.try_with_tx` | Outbound HTTP allowed (`ctx.http`) | `http::Response<Body>` |
| Slice / query read-only state | `#[spacetimedb::view]` | Read-only transaction | Forbidden | `Vec<T>` or `Option<T>` |
| Broadcast transient events | Event Table insert in reducer | Covered by reducer transaction | N/A | Broadcast to subscribers |

---

## Critical v2.x Invariants (Never Violate)

- **NO Reducer Callbacks:** `conn.reducers.on_<reducer>()` is completely removed in 2.0. Use **Event Tables** (`#[table(accessor = ..., event)]`) to notify subscribers about occurrences.
- **NO `light_mode` or `CallReducerFlags`:** Both were eliminated with the removal of 1.x reducer broadcast callbacks.
- **Table Accessors vs Canonical Names:** Always specify `accessor = <name>` in table attributes (e.g., `#[table(accessor = player, public)]`). The `name =` attribute is reserved for overriding the canonical SQL table name.
- **Views have no full-table `.iter()`:** In SpacetimeDB 2.x, views do NOT support `ctx.db.<table>().iter()`. Use `ctx.db.<table>().count()` on `AnonymousViewContext` for total counts, or index filters (`.filter(&val)`) for subset iteration.
- **Procedures must not hold transactions during I/O:** Never invoke `ctx.http` inside `ctx.with_tx(...)`. Perform network I/O before or after transaction blocks.

---

## Quick Reference / Cheatsheet

### 1. Minimal Server Module (`Cargo.toml`)
```toml
[package]
name = "my-module"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
spacetimedb = { version = "2.11", features = ["unstable"] }
log = "0.4"
```

### 2. Table & Event Table
```rust
use spacetimedb::{table, Identity};

#[table(accessor = player, public)]
pub struct Player {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub identity: Identity,
    #[index(btree)]
    pub score: u32,
}

// Event table: published on commit, then immediately deleted
#[table(accessor = combat_event, public, event)]
pub struct CombatEvent {
    pub target: Identity,
    pub damage: u32,
}
```

### 3. Transactional Reducer
```rust
use spacetimedb::{reducer, ReducerContext, Table};

#[reducer]
pub fn attack(ctx: &ReducerContext, target_id: u64, damage: u32) -> Result<(), String> {
    // Validate attacker
    if ctx.db.player().identity().find(&ctx.sender()).is_none() {
        return Err("Attacker not registered".to_string());
    }

    let mut player = ctx.db.player().id().find(&target_id)
        .ok_or_else(|| "Target player not found".to_string())?;

    let target_identity = player.identity;
    player.score = player.score.saturating_sub(damage);
    ctx.db.player().id().update(player);

    // Publish transient event to subscribers
    ctx.db.combat_event().insert(CombatEvent {
        target: target_identity,
        damage,
    });

    Ok(())
}
```

### 4. Procedure with Outbound HTTP
```rust
use spacetimedb::{procedure, ProcedureContext, Table};

#[procedure]
pub fn verify_and_credit(ctx: &mut ProcedureContext, user_id: u64) -> Result<bool, String> {
    // 1. External side effect (outbound HTTP) outside of any transaction
    let response = ctx.http.get("https://auth.service.internal/verify")
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if response.status() != 200 {
        return Ok(false);
    }

    // 2. Manual transaction boundary for database mutation
    ctx.with_tx(|tx| {
        if let Some(mut p) = tx.db.player().id().find(&user_id) {
            p.score += 100;
            tx.db.player().id().update(p);
        }
    });

    Ok(true)
}
```

### 5. Inbound HTTP Handler & Router
```rust
use spacetimedb::http::{handler, router, Body, HandlerContext, Request, Response, Router};

#[handler]
fn webhook_handler(ctx: &mut HandlerContext, req: Request) -> Response {
    let body_bytes = req.into_body().into_bytes();
    
    ctx.with_tx(|tx| {
        // transactional processing of incoming payload
    });

    Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(Body::from_bytes(b"{\"status\":\"ok\"}".as_slice()))
        .unwrap()
}

#[router]
fn http_routes() -> Router {
    Router::new().post("/webhook", webhook_handler)
}
```

### 6. Views (Read-Only Slices)
```rust
use spacetimedb::{view, AnonymousViewContext, ViewContext, SpacetimeType};

#[derive(SpacetimeType)]
pub struct CountRow { pub count: u64 }

#[view(accessor = total_players, public)]
pub fn total_players(ctx: &AnonymousViewContext) -> Vec<CountRow> {
    vec![CountRow { count: ctx.db.player().count() }]
}

#[view(accessor = my_player, public)]
pub fn my_player(ctx: &ViewContext) -> Option<Player> {
    ctx.db.player().identity().find(ctx.sender())
}
```

---

## Detailed Reference Documentation

- [Tables & Schemas](references/tables.md)
- [Reducers & Transactions](references/reducers.md)
- [Procedures & Outbound HTTP](references/procedures.md)
- [Inbound HTTP Handlers & Routers](references/http.md)
- [Views & Subscriptions](references/views.md)
- [Rust Client SDK & Bindings](references/client-rust.md)
- [CLI Workflows & Native MCP](references/cli-and-mcp.md)
- [1.x to 2.x Upgrade Guide & Breaking Changes](references/v2-breaking-changes.md)
