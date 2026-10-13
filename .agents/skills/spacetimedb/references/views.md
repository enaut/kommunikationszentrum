# Views & Metrics in SpacetimeDB v2.11

Views are read-only query functions executed server-side inside an isolated transaction. They allow modules to project, filter, join, and compute aggregate slices of data before publishing them to client subscriptions.

---

## 1. Defining Views

Views are annotated with `#[spacetimedb::view(accessor = <name>, public)]`:

```rust
use spacetimedb::{view, SpacetimeType, ViewContext, AnonymousViewContext};

#[derive(SpacetimeType, Clone, Debug)]
pub struct PlayerSummary {
    pub id: u64,
    pub name: String,
    pub level: u32,
}

// 1. Single-row view (Option<T>)
#[view(accessor = my_player, public)]
pub fn my_player(ctx: &ViewContext) -> Option<PlayerSummary> {
    ctx.db.player()
        .identity()
        .find(ctx.sender())
        .map(|p| PlayerSummary { id: p.id, name: p.name, level: p.level })
}

// 2. Multi-row view (Vec<T>)
#[view(accessor = top_players, public)]
pub fn top_players(ctx: &AnonymousViewContext) -> Vec<PlayerSummary> {
    ctx.db.player()
        .score()
        .filter(1000..)
        .map(|p| PlayerSummary { id: p.id, name: p.name, level: p.level })
        .collect()
}
```

---

## 2. Context Types: `AnonymousViewContext` vs `ViewContext`

SpacetimeDB provides two distinct context types for views. Choosing the correct one is critical for performance:

### `AnonymousViewContext` (Global & Materialized)
* Does **not** have access to caller identity (`ctx.sender()`).
* **Shared Materialization:** SpacetimeDB executes the view once and shares the cached result across all client subscriptions.
* Ideal for: Global leaderboards, server statistics, public catalogues, aggregate counters.

### `ViewContext` (Sender-Specific)
* Has access to `ctx.sender()` (the `Identity` of the subscribing client).
* Evaluated per-subscriber.
* Ideal for: Private player inventories, user profiles, personal inbox messages.
* Can be converted to anonymous if needed via `ctx.as_anonymous()`.

---

## 3. Strict 2.x Architectural Rules for Views

### Rule 1: No Full-Table `.iter()` in Views
In SpacetimeDB 2.x, `ViewHandle` does **not** implement `.iter()`. You cannot perform a full unindexed table scan inside a view function:
```rust
// ❌ COMPILE ERROR in SpacetimeDB 2.x views:
for row in ctx.db.player().iter() { ... }
```

### Rule 2: Constant-Time Row Counts
To calculate global table counts:
- Use `ctx: &AnonymousViewContext`.
- Call `ctx.db.<table>().count()`.
This is an $O(1)$ table metadata lookup that SpacetimeDB materializes once and broadcasts to all subscribers:

```rust
#[derive(SpacetimeType, Clone)]
pub struct CountRow { pub count: u64 }

#[spacetimedb::view(accessor = total_accounts, public)]
pub fn total_accounts(ctx: &AnonymousViewContext) -> Vec<CountRow> {
    vec![CountRow {
        count: ctx.db.account().count(),
    }]
}
```

### Rule 3: Filtered Iteration via Indexes
Index range and equality filters (`.filter(&val)`) return iterators that **do** support iteration and `.count()`. Always use indexed columns when filtering rows in views:

```rust
#[derive(SpacetimeType, Clone)]
pub struct TopicCount { pub topic_id: u64, pub count: u64 }

#[spacetimedb::view(accessor = topic_message_counts, public)]
pub fn topic_message_counts(ctx: &AnonymousViewContext) -> Vec<TopicCount> {
    let mut results = Vec::new();
    for topic in ctx.db.topic().id().filter(0..) {
        // Filter on indexed column `topic_id`:
        let count = ctx.db.message().topic_id().filter(&topic.id).count() as u64;
        results.push(TopicCount { topic_id: topic.id, count });
    }
    results
}
```

### Rule 4: Never Reducer-Cache Aggregate Counts
Never store aggregate metrics (like `total_users`, `message_count`) in persistent table columns updated during mutation reducers. This creates write contention, transaction rollbacks, and cache staleness. Expose aggregates via `AnonymousViewContext` views instead.
