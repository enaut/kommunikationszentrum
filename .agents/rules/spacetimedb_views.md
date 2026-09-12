---
name: spacetimedb-views
description: Guidelines for implementing SpacetimeDB views, aggregations, and metrics
trigger: always_on
---

# SpacetimeDB Views & Metrics Guidelines

### 1. No Reducer-Cached Counts
Never cache derived aggregate metrics (e.g. `total_accounts`, `total_messages`, category counts) in persistent table columns (such as `AccountConfig`) that get updated during mutation reducers. This causes massive write amplification and stale cache bugs.

### 2. Full Table `.iter()` Restriction in Views
In SpacetimeDB 2.x, `ViewHandle` does NOT support `.iter()`. Do not attempt full-table iteration in views.

### 3. Constant-Time Counts via `AnonymousViewContext`
To compute global totals (e.g. total accounts, total messages):
- Use `#[spacetimedb::view(accessor = <name>, public)]`.
- Use `ctx: &spacetimedb::AnonymousViewContext`.
- Call `ctx.db.<table>().count()`. This is an $O(1)$ table metadata lookup that SpacetimeDB materializes once and shares across all client subscriptions.

```rust
use spacetimedb::{view, AnonymousViewContext, SpacetimeType};

#[derive(SpacetimeType)]
pub struct CountRow { pub count: u64 }

#[view(accessor = total_accounts, public)]
pub fn total_accounts(ctx: &AnonymousViewContext) -> Vec<CountRow> {
    vec![CountRow { count: ctx.db.account().count() }]
}
```

### 4. Filtered Counts on Indexed Columns
Index range/equality filters (e.g. `ctx.db.<table>().<column>().filter(&val)`) return iterators that DO support `.count()`. Use these for category- or status-filtered metrics.
