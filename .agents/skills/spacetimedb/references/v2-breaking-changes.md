# Migrating from 1.x to 2.x/2.11 & Forbidden Antipatterns

SpacetimeDB v2.0+ introduced breaking architectural changes to eliminate security vulnerabilities, prevent memory leaks, and simplify distributed state synchronization.

---

## 1. Major Breaking Changes Overview

| Feature | 1.x Behavior (Deprecated / Removed) | 2.0–2.11+ Behavior (Current Standard) |
|---|---|---|
| **Reducer Callbacks** | `conn.reducers.on_<reducer>(...)` broadcast reducer arguments to all subscribers of touched rows. | **REMOVED.** Use **Event Tables** (`#[table(event)]`) for notifications, or `_then()` for caller response. |
| **Reducer Flags** | `CallReducerFlags::NoSuccessNotify`, `set_reducer_flags()`. | **REMOVED.** No flags needed. |
| **`light_mode`** | Client configuration to disable reducer broadcasts. | **REMOVED.** Reducer broadcasts do not exist. |
| **Table Macro Argument** | `#[table(name = my_table)]` set the code accessor. | `#[table(accessor = my_table)]` sets the code accessor. `name =` overrides the raw SQL name. |
| **Client Event Tag** | `Event::Reducer` with arguments broadcast to all subscribers; `UnknownTransaction`. | `Event::Reducer` sent only to caller; other subscribers receive `Event::Transaction`. |
| **Side Effects / External HTTP** | Not supported in server modules. | Supported via `#[spacetimedb::procedure]` and `ctx.http`. |
| **Inbound Webhooks** | Required external reverse proxy. | Supported natively via `#[spacetimedb::http::handler]` and `#[router]`. |
| **Read Consistency** | Unconfirmed speculative reads sent immediately. | **Confirmed reads enabled by default:** Updates sent only after durable commitlog write. |
| **View Table Iteration** | Unrestricted `.iter()` in views. | **`.iter()` unsupported on `ViewHandle`:** Use `count()` or index filters. |

---

## 2. Reducer Broadcasts → Event Tables Migration

### ❌ 1.x Antipattern (Will Not Compile)
```rust
// 1.0 Server (Arguments were automatically broadcast to subscribers)
#[spacetimedb::reducer]
fn deal_damage(ctx: &ReducerContext, target: Identity, amount: u32) {
    // mutations...
}

// 1.0 Client (Global callback listening to any deal_damage call)
conn.reducers.on_deal_damage(|ctx, target, amount| {
    play_animation(target, amount);
});
```

### ✅ 2.x Idiomatic Pattern (Event Tables)
```rust
// 2.x Server: Declare an Event Table
#[spacetimedb::table(accessor = damage_event, public, event)]
pub struct DamageEvent {
    pub target: Identity,
    pub amount: u32,
}

#[spacetimedb::reducer]
pub fn deal_damage(ctx: &ReducerContext, target: Identity, amount: u32) -> Result<(), String> {
    // 1. Mutate persistent game state...
    
    // 2. Publish transient event to all subscribers
    ctx.db.damage_event().insert(DamageEvent { target, amount });
    Ok(())
}

// 2.x Client: Subscribe to event table and listen on insert
ctx.subscription_builder()
    .add_query(|q| q.from.damage_event())
    .subscribe();

conn.db.damage_event().on_insert(|ctx, event| {
    play_animation(event.target, event.amount);
});
```

---

## 3. Calling Reducers & Awaiting Results

If only the *calling* client needs to know whether a reducer succeeded or failed:

### 1.x Approach
```rust
// 1.0 Fire and hope
conn.reducers.deal_damage(target, amount).unwrap();
```

### 2.x Approach (`_then` callback)
```rust
// 2.0 Correlated per-call callback
conn.reducers.deal_damage_then(target, amount, |ctx, result| {
    match result {
        Ok(Ok(())) => log::info!("Reducer succeeded!"),
        Ok(Err(err)) => log::warn!("Reducer failed with logic error: {err}"),
        Err(internal) => log::error!("Host internal error: {internal:?}"),
    }
}).unwrap();
```

---

## 4. Table Declaration Migration

### ❌ 1.x Antipattern
```rust
#[spacetimedb::table(
    name = player,
    public,
    index(name = by_score, btree(columns = [score]))
)]
pub struct Player { ... }
```

### ✅ 2.x Standard
```rust
#[spacetimedb::table(
    accessor = player,
    public,
    index(accessor = by_score, btree(columns = [score]))
)]
pub struct Player { ... }
```

---

## 5. Prohibited Antipatterns Quick Checklist

When writing or reviewing SpacetimeDB code, reject the following patterns immediately:

1. **`conn.reducers.on_<name>()`:** Does not exist. Use Event Tables.
2. **`ctx.http` inside `ctx.with_tx(...)`:** Prohibited. Perform HTTP calls outside transactions.
3. **`ctx.db.<table>().iter()` inside views:** Prohibited. Full table iteration is disabled in views.
4. **Caching counts in table columns:** Prohibited. Use `AnonymousViewContext` views with `ctx.db.<table>().count()`.
5. **Omitting explicit subscriptions for Event Tables:** Event tables are not subscribed by `subscribe_to_all_tables()`; they must have an explicit query in the subscription builder.
6. **Writing to captured `Cell`/`RefCell` in `with_tx` closures:** Prohibited due to automatic transaction conflict retries.
