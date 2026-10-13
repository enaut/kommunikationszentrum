# Reducers & Transactions in SpacetimeDB v2.11

Reducers are the primary write mechanism in SpacetimeDB. They execute transactional business logic inside the WebAssembly runtime.

---

## 1. Reducer Signatures

Every reducer is annotated with `#[spacetimedb::reducer]`.

```rust
use spacetimedb::{reducer, ReducerContext, Identity, SpacetimeType};

#[derive(SpacetimeType)]
pub struct MoveCommand {
    pub x: f32,
    pub y: f32,
}

#[reducer]
pub fn move_player(ctx: &ReducerContext, cmd: MoveCommand) -> Result<(), String> {
    let sender = ctx.sender();
    // Validate and update player position...
    Ok(())
}
```

### Signature Rules:
1. **First argument:** Must be `&ReducerContext` (by convention named `ctx`).
2. **Subsequent arguments:** Zero or more arguments implementing `SpacetimeType`, `Serialize`, and `Deserialize`.
3. **Return Type:** Must return either `()` or `Result<(), E>` where `E: std::fmt::Display`.
4. **Visibility:** Standard Rust visibility (`pub` or private). The macro handles export registration.

---

## 2. ReducerContext Capabilities

The `ReducerContext` struct provides transactional access to the database and caller metadata:

| Method / Field | Type | Description |
|---|---|---|
| `ctx.sender()` | `Identity` | Identity of the client invoking the reducer. For scheduled or init reducers, equals module identity. |
| `ctx.connection_id()` | `Option<ConnectionId>` | Specific client connection. `None` for internal or scheduled calls. |
| `ctx.timestamp` | `Timestamp` | Microsecond timestamp at which the reducer invocation started. Monotonic within the transaction. |
| `ctx.database_identity()` | `Identity` | The current module's unique database identity. |
| `ctx.db` | `Local` | Handle to the database. Access table methods via `ctx.db.<accessor>()`. |
| `ctx.new_uuid_v4()` | `anyhow::Result<Uuid>` | Random UUID v4 using host RNG. |
| `ctx.new_uuid_v7()` | `anyhow::Result<Uuid>` | Monotonic, sortable UUID v7 combining timestamp, counter, and random bytes. |

---

## 3. Transaction Semantics & Error Handling

* **ACID Isolation:** Reducers run in serializable / snapshot-isolated transactions.
* **Commit on Success:** If a reducer returns `Ok(())` or `()`, all table mutations and event table insertions commit atomically to durable storage and are broadcast to subscribers.
* **Rollback on Error or Panic:** If a reducer returns `Err(err)` or panics:
  * All database changes are rolled back.
  * No event table rows are broadcast.
  * The error message is returned only to the calling client (via client-side `_then` callback or promise rejection).
* **Logging:** Use standard `log` macros (`log::info!`, `log::warn!`, `log::error!`). Module logs are streamed to `spacetime logs <DATABASE>`.

---

## 4. Lifecycle Reducers

Modules can define special lifecycle hooks. Each module may define at most one reducer of each lifecycle type:

### 1. `init` Reducer
Runs once when the module is initially published, or whenever the database is cleared.

```rust
use spacetimedb::{reducer, ReducerContext, Table};

#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    log::info!("Initializing database instance: {:?}", ctx.database_identity());
    
    // Seed initial configuration or default rows
    // Seed initial scheduled timers
}
```

### 2. `client_connected` Reducer
Triggered automatically whenever a client opens a connection:

```rust
#[reducer(client_connected)]
pub fn on_client_connected(ctx: &ReducerContext) {
    let client = ctx.sender();
    log::info!("Client connected: {:?}", client);
    // Note: returning Err() or panicking here will disconnect the client immediately.
}
```

### 3. `client_disconnected` Reducer
Triggered automatically when a client disconnects:

```rust
#[reducer(client_disconnected)]
pub fn on_client_disconnected(ctx: &ReducerContext) {
    let client = ctx.sender();
    log::info!("Client disconnected: {:?}", client);
}
```

---

## 5. Scheduled Reducers (Timers & Game Loops)

Scheduled reducers run automatically at a future time or on a recurring interval.

### Step 1: Define the Timer Table
```rust
use spacetimedb::{table, ScheduleAt};

#[table(accessor = tick_schedule, scheduled(game_tick))]
pub struct TickSchedule {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    pub tick_rate: u32,
}
```

### Step 2: Define the Reducer
The reducer takes `&ReducerContext` and the schedule row struct as its second argument:

```rust
use spacetimedb::{reducer, ReducerContext, Table};

#[reducer]
pub fn game_tick(ctx: &ReducerContext, schedule: TickSchedule) -> Result<(), String> {
    // Execute scheduled game loop logic...
    Ok(())
}
```

### Step 3: Schedule Execution
Insert into the timer table using `ScheduleAt::Time` (one-shot) or `ScheduleAt::Interval` (loop):

```rust
use std::time::Duration;
use spacetimedb::{reducer, ReducerContext, Table, ScheduleAt};

#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    // Run every 100 milliseconds
    ctx.db.tick_schedule().insert(TickSchedule {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(Duration::from_millis(100).into()),
        tick_rate: 10,
    });
}
```

### Scheduler Notes:
* **Canceling:** To cancel a scheduled reducer, simply delete its row from the timer table: `ctx.db.tick_schedule().scheduled_id().delete(&id)`.
* **Non-blocking Concurrency (v2.10.1+):** Scheduled tasks submit concurrently to the worker pool without head-of-line blocking.
* **Security Restricting:** Scheduled reducers can also be invoked by external clients unless you verify the caller identity:
  ```rust
  if ctx.sender() != ctx.database_identity() {
      return Err("Scheduled reducer cannot be invoked externally".to_string());
  }
  ```

---

## 6. Inter-Reducer Calls

Reducers can directly invoke other reducers within the same module by passing `ctx`:

```rust
#[reducer]
pub fn outer_reducer(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    inner_helper(ctx, id)?;
    Ok(())
}

fn inner_helper(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    // Shares the outer reducer's transaction scope
    Ok(())
}
```
All mutations occur within the single parent transaction and commit or rollback together.
