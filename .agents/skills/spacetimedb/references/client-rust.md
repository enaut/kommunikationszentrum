# Rust Client SDK & Generated Bindings in SpacetimeDB v2.11

The SpacetimeDB Rust client SDK connects natively over WebSockets (using the `v3.bsatn.spacetimedb` subprotocol), synchronizes local relational table caches, and invokes reducers and procedures.

---

## 1. Generating Module Bindings

Generate type-safe client bindings from your server module:

```bash
spacetime generate --lang rust -p server -o client/src/module_bindings/
cargo fmt -p client
```

This creates the module bindings containing:
* `DbConnection`: Main client connection handle.
* Table structs and table cache handles (`conn.db.<table_accessor>()`).
* Reducer dispatchers (`conn.reducers.<reducer>()` and `conn.reducers.<reducer>_then()`).
* Typed query builders for subscriptions (`q.from.<table_accessor>()`).

---

## 2. Establishing a Connection

```rust
use std::sync::Arc;
use module_bindings::DbConnection;

fn main() -> anyhow::Result<()> {
    let conn = DbConnection::builder()
        .with_uri("ws://127.0.0.1:3000")
        .with_database_name("my_database")
        .on_connect(|ctx, identity, token| {
            log::info!("Connected! Identity: {identity}");
            
            // Subscribe to tables upon connecting
            ctx.subscription_builder()
                .on_applied(|_ctx| log::info!("Initial subscription sync applied!"))
                .on_error(|_ctx, err| log::error!("Subscription error: {err}"))
                .add_query(|q| q.from.player())
                .add_query(|q| q.from.combat_event()) // Selective subscription to an event table
                .subscribe();
        })
        .on_connect_error(|_ctx, err| {
            log::error!("Connection error: {err}");
        })
        .on_disconnect(|_ctx| {
            log::warn!("Disconnected from SpacetimeDB");
        })
        .build()?;

    // Run connection event loop (or run in background task)
    conn.run()?;
    Ok(())
}
```

---

## 3. Subscriptions & Query Builders

In SpacetimeDB 2.0+, client subscriptions use client-assigned `QuerySetId` and typed query builders:

```rust
// 1. Subscribe to specific tables (selective subscription)
ctx.subscription_builder()
    .add_query(|q| q.from.player())
    .add_query(|q| q.from.world_state())
    .add_query(|q| q.from.combat_event()) // Selective subscription to an event table
    .subscribe();

// 2. Subscribe to all public tables (automatically includes public event tables)
ctx.subscription_builder()
    .subscribe_to_all_tables();

// 3. Observing Event Tables:
// Public event tables are automatically included in subscribe_to_all_tables(),
// or can be subscribed to selectively via add_query().
// Because event table rows are ephemeral and never cached, observe them via on_insert callbacks:
conn.db.combat_event().on_insert(|_ctx, event| {
    log::info!("Combat event received: {:?}", event);
});
```

---

## 4. Table Callbacks & Cache Inspection

Clients maintain an in-memory cache of subscribed tables:

```rust
// 1. Read directly from the client cache
if let Some(player) = conn.db.player().id().find(&player_id) {
    log::info!("Found player: {}, score: {}", player.name, player.score);
}

// 2. Iterate local cache rows
for player in conn.db.player().iter() {
    log::info!("Player in cache: {}", player.name);
}

// 3. React to table mutations
conn.db.player().on_insert(|ctx, row| {
    log::info!("New player added: {:?}", row.name);
});

conn.db.player().on_update(|ctx, old_row, new_row| {
    log::info!("Player score updated from {} to {}", old_row.score, new_row.score);
});

conn.db.player().on_delete(|ctx, row| {
    log::info!("Player deleted: {:?}", row.name);
});
```

---

## 5. Identifying the Mutator (`Event::Reducer` vs `Event::Transaction`)

In 1.x, reducer callbacks broadcast all arguments to all clients. In 2.0, reducer arguments are not broadcast to third-party subscribers:

* **The Calling Client:** Receives `Event::Reducer(reducer_event)` detailing the reducer name, timestamp, and status.
* **Other Subscribers:** Receive `Event::Transaction` indicating rows were mutated, without leaking caller arguments.

```rust
conn.db.player().on_insert(|ctx, row| {
    match &ctx.event {
        module_bindings::Event::Reducer(reducer_event) => {
            // THIS connection caused the change via a reducer
            log::info!("Our reducer caused this insert: {:?}", reducer_event.reducer);
        }
        module_bindings::Event::Transaction => {
            // Another client or a timer caused this change
            log::info!("External transaction caused this insert");
        }
        _ => {}
    }
});
```

---

## 6. Invoking Reducers

### Fire-and-Forget
Sends the reducer invocation without blocking for confirmation:
```rust
conn.reducers.move_player(10.5, 20.0).unwrap();
```

### Per-Call Callback (`_then`)
Correlates the response specifically to your request:
```rust
conn.reducers.move_player_then(10.5, 20.0, |ctx, result| {
    match result {
        Ok(Ok(())) => log::info!("Move confirmed on server!"),
        Ok(Err(err)) => log::warn!("Move rejected by server logic: {err}"),
        Err(internal) => log::error!("Host internal error: {internal:?}"),
    }
}).unwrap();
```
