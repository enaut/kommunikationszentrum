# Tables & Schemas in SpacetimeDB v2.11

In SpacetimeDB, relational state is defined using standard Rust `struct` definitions decorated with `#[spacetimedb::table(...)]`.

---

## 1. Table Declaration & Attributes

```rust
use spacetimedb::{table, Identity, Timestamp};

#[table(
    accessor = user_account,
    public,
    index(accessor = by_status_and_created, btree(columns = [status, created_at]))
)]
pub struct UserAccount {
    #[primary_key]
    #[auto_inc]
    pub id: u64,

    #[unique]
    pub email: String,

    pub status: String,

    #[index(btree)]
    pub score: u32,

    pub created_at: Timestamp,
}
```

### Table Macro Arguments

* **`accessor = <ident>` (Required):** Defines the identifier used in Rust code to access the table handle (e.g., `ctx.db.user_account()`).
* **`name = "<sql_name>"` (Optional):** Overrides the canonical table name in the database/SQL catalog. If omitted, SpacetimeDB converts the accessor identifier using the module's case-conversion policy (default: `snake_case`).
* **`public` / `private`:** Tables are `private` by default. `public` tables allow connected clients to subscribe to and query rows.
* **`event`:** Declares the table as an **Event Table** (see section 5).
* **`scheduled(<reducer_or_procedure>)`:** Marks the table as a **Schedule Table** (see section 6).
* **`index(...)`:** Declares composite (multi-column) indexes.

---

## 2. Column Attributes & Constraints

| Attribute | Meaning | Generated Accessor Behavior |
|---|---|---|
| `#[primary_key]` | Primary key column. Enforces uniqueness. Implies `#[unique]`. | `ctx.db.<table>().<column>().find(&val)` |
| `#[unique]` | Unique constraint. Generates a unique index. | `ctx.db.<table>().<column>().find(&val)`, `update(row)`, `delete(&val)` |
| `#[auto_inc]` | Auto-incrementing sequence on numeric columns. | When inserted with `0`, assigns the next monotonic value. |
| `#[index(btree)]` | Single-column B-tree index. | `ctx.db.<table>().<column>().filter(range_or_val)` |

### Restrictions & Best Practices:
* Do not apply `#[index(btree)]` to a field that already has `#[unique]` or `#[primary_key]` (they already generate unique index handles).
* `#[auto_inc]` does not imply `#[primary_key]`; combine `#[primary_key]` and `#[auto_inc]` if you want auto-incrementing primary IDs.
* Never insert values manually above the sequence in `#[auto_inc]` unique columns, as the sequence will eventually collide and fail.

---

## 3. Table Operations Inside Reducers

Obtain a table handle via `ctx.db.<accessor>()`.

```rust
use spacetimedb::Table; // Ensure Table trait is in scope

// 1. Insertion
ctx.db.user_account().insert(UserAccount { ... }); // Panics on constraint violation
let res = ctx.db.user_account().try_insert(UserAccount { ... }); // Returns Result

// 2. Full-table scan (allowed in reducers, forbidden in views)
for user in ctx.db.user_account().iter() {
    log::info!("User: {:?}", user.id);
}

// 3. Fast O(1) row count
let total = ctx.db.user_account().count();

// 4. Unique column lookup & update
if let Some(mut user) = ctx.db.user_account().email().find(&"alice@example.com".to_string()) {
    user.score += 10;
    ctx.db.user_account().id().update(user);
}

// 5. Unique column deletion
let existed = ctx.db.user_account().email().delete(&"bob@example.com".to_string());

// 6. Indexed range filter
for user in ctx.db.user_account().score().filter(100..=500) {
    log::info!("Player in tier: {:?}", user.email);
}

// 7. Multi-column index lookup
for user in ctx.db.user_account().by_status_and_created().filter(("active".to_string(), start_time..)) {
    // ...
}
```

---

## 4. Supported Column Types

SpacetimeDB supports all standard primitive types and any type deriving `#[derive(spacetimedb::SpacetimeType)]`:

* Integers: `u8`, `u16`, `u32`, `u64`, `u128`, `i8`, `i16`, `i32`, `i64`, `i128`
* Floats: `f32`, `f64`
* Boolean: `bool`
* Text: `String`, `Box<str>`
* Binary: `Vec<u8>`, `Box<[u8]>`
* SpacetimeDB primitives:
  * `spacetimedb::Identity` (32-byte client identity)
  * `spacetimedb::ConnectionId` (128-bit connection ID)
  * `spacetimedb::Timestamp` (microsecond Unix epoch timestamp)
  * `spacetimedb::TimeDuration` (microsecond duration)
  * `spacetimedb::ScheduleAt` (scheduled timestamp or interval)
* Options & Enums: `Option<T>`, user-defined enums deriving `SpacetimeType`.

---

## 5. Event Tables (Transient Notifications)

In SpacetimeDB 2.0+, global reducer broadcast callbacks were eliminated. Transient messaging (e.g., chat messages, combat damage floaters, UI toast events) is modeled through **Event Tables**:

```rust
#[spacetimedb::table(accessor = combat_event, public, event)]
pub struct CombatEvent {
    pub target: Identity,
    pub damage: u32,
    pub source: String,
}
```

### Event Table Lifecycle & Guarantees:
1. **Transaction-Bound:** During reducer execution, event tables behave like standard tables: you can insert rows, read them, and apply constraints.
2. **Auto-Purge on Commit:** When the transaction commits, SpacetimeDB publishes inserted rows to subscribed clients, and immediately removes them from the table.
3. **Empty Between Transactions:** Event tables are always empty outside of an active transaction. On clients, `iter()` is empty and `count()` is always 0.
4. **Subscriptions:** `subscribe_to_all_tables()` automatically includes public event tables (observed via `on_insert` callbacks). Explicit queries via query builder or SQL are only required when using selective subscriptions:
   ```rust
   ctx.subscription_builder().add_query(|q| q.from.combat_event()).subscribe();
   ```
5. **Irreversible Flag:** Once a table is published with `event`, it cannot be changed to a persistent table (or vice versa) without dropping and recreating the table.

---

## 6. Scheduled Timer Tables

SpacetimeDB supports transactional timers by declaring a table with `scheduled(<func>)`:

```rust
use spacetimedb::{table, ScheduleAt};

#[table(accessor = cleanup_timer, scheduled(cleanup_stale_data))]
pub struct CleanupTimer {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,

    pub scheduled_at: ScheduleAt,

    pub batch_size: u32,
}
```

### Timer Table Requirements:
* Must contain `scheduled_id: u64` (marked `#[primary_key]` and `#[auto_inc]`).
* Must contain `scheduled_at: ScheduleAt`.
* Additional custom fields can be included to pass parameters to the scheduled function.
* Inserting a row schedules the callback; deleting the row cancels the scheduled invocation.
* The scheduled function receives `(ctx: &ReducerContext, arg: CleanupTimer)`.

---

## 7. Module Settings

Define module-level settings using `#[spacetimedb::settings]`:

```rust
use spacetimedb::{settings, CaseConversionPolicy};

#[settings]
const CASE_CONVERSION_POLICY: CaseConversionPolicy = CaseConversionPolicy::SnakeCase;
```
