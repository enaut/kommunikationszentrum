# Admin Frontend Restructure Plan

This document describes the planned restructure of the `admin` Dioxus frontend, replacing the
current single-file monolith with a proper multi-page application split by role.

## Goals

- A dedicated **Message Categories** management view (list, add, remove) instead of the broken
  per-user button on the accounts list.
- A **My Subscriptions** view as the default for all users: browse categories, subscribe and
  unsubscribe with a simple interface.
- **Admin-only views**: full control over categories, all member subscriptions, and debug
  information.
- Split `main.rs` into focused, maintainable modules.

## What is wrong today

- All UI lives in a single `main.rs` (~400 lines).
- The only rendered view is an accounts list with a placeholder "Thema hinzufügen" button
  that passes a hardcoded category name — it does not work as intended.
- The `message_categories` and `subscriptions` tables are never subscribed to or displayed.
- No role-based view separation: every authenticated user sees the same admin-style layout.

---

## Phase 1 — Server changes (`server/src/mailing.rs`)

### New reducers required

| Reducer | Signature | Authorization |
|---|---|---|
| `remove_message_category` | `(category_id: u64)` | admin only |
| `remove_subscription` | `(subscription_id: u64)` | self or admin |

### Table visibility changes required

| Table | Change | Reason |
|---|---|---|
| `message_categories` | Add `public` attribute | All users need to see the category list to subscribe |
| `subscriptions` | Add `#[spacetimedb::client_visibility_filter]` | Users see only their own rows; admins see all |

After these changes, republish and regenerate bindings:

```bash
spacetime publish --project-path server kommunikation
spacetime generate --project-path server --lang rust --out-dir admin/src/module_bindings
```

---

## Phase 2 — `module_bindings/dioxus.rs` additions

The hand-written Dioxus integration layer requires the following additions.

### `TableSignals` struct

Add `message_categories` and `subscriptions` signals alongside the existing ones:

```rust
pub struct TableSignals {
    pub account: SyncSignal<Vec<Account>>,
    pub admin_identities: SyncSignal<Vec<AdminIdentity>>,
    pub visible_accounts: SyncSignal<Vec<Account>>,
    pub message_categories: SyncSignal<Vec<MessageCategory>>,  // new
    pub subscriptions: SyncSignal<Vec<Subscription>>,           // new
}
```

### New hooks to add

```rust
pub fn use_table_message_categories() -> SyncSignal<Vec<MessageCategory>>
pub fn use_table_subscriptions() -> SyncSignal<Vec<Subscription>>
pub fn use_reducer_remove_message_category() -> impl Fn(u64) -> Result<()>
pub fn use_reducer_remove_subscription() -> impl Fn(u64) -> Result<()>
pub fn use_is_admin() -> bool   // checks current SpacetimeDB identity against admin_identities
```

`use_is_admin` derives its result from `use_table_admin_identities()` and the current identity
from `use_connection_state()`.

### Subscription queries in `AuthenticatedApp`

Expand the `use_subscription` call to include the two new tables:

```rust
use_subscription(&[
    "SELECT * FROM visible_accounts",
    "SELECT * FROM admin_identities",
    "SELECT * FROM message_categories",
    "SELECT * FROM subscriptions",
]);
```

---

## Phase 3 — Frontend file structure

```
admin/src/
├── main.rs                   # App entry, auth state routing, AuthenticatedApp shell
├── config.rs                 # Unchanged
├── oauth.rs                  # Unchanged
├── module_bindings/          # Generated types + hand-written dioxus.rs
├── router.rs                 # ActiveView enum + navigation helpers
├── components/
│   ├── mod.rs
│   └── navbar.rs             # Bootstrap navbar, view switching, logout button
└── pages/
    ├── mod.rs
    ├── subscriptions.rs      # "My Subscriptions" — default view, all users
    ├── categories.rs         # "Message Categories" — admin CRUD
    ├── members.rs            # "Members" — admin view of accounts + their subscriptions
    └── debug.rs              # "Debug & Status" — admin: connection info, identities, logs
```

---

## Component responsibilities

### `main.rs` (slim, ~80 lines)

- `main()` → `dioxus::launch(App)`
- `App`: loads config, sets up OAuth, routes between
  `LoginPage / AuthenticatingPage / AuthenticatedApp / ErrorPage`
- `AuthenticatedApp`: creates SpacetimeDB context, subscribes to all tables, owns
  `active_view: Signal<ActiveView>`, renders `Navbar` + the active page
- `LoginPage`, `AuthenticatingPage`, `ErrorPage` stay here — they are small and auth-specific

### `router.rs`

```rust
#[derive(Clone, PartialEq)]
pub enum ActiveView {
    MySubscriptions,  // default for all users
    Categories,       // admin only
    Members,          // admin only
    Debug,            // admin only
}
```

Also exports `use_is_admin()`.

### `components/navbar.rs`

- Bootstrap navbar with the application title
- "My Subscriptions" link always visible
- "Categories", "Members", "Debug" links visible only when `use_is_admin()` is true
- User dropdown with logout

### `pages/subscriptions.rs` — default for all users

- Lists all `message_categories` as cards
- For each category: shows subscription status by checking the `subscriptions` table
  (match on `subscriber_account_id` or `subscriber_email`)
- Toggle button per category: calls `add_subscription` or `remove_subscription`
- Designed for non-technical members; no raw IDs or debug data visible

### `pages/categories.rs` — admin only

- Table of all `MessageCategory` rows: name, email address, description, active badge
- Inline "Add Category" form (name, email address, description) → `add_message_category`
- Delete button per row → `remove_message_category`

### `pages/members.rs` — admin only

- Table of all `visible_accounts`
- Per-member expandable section or modal listing that member's current subscriptions
- Admins can add or remove subscriptions on behalf of any member

### `pages/debug.rs` — admin only

- Moves the current `ConnectionStatusCard` here
- SpacetimeDB connection state, current identity, raw JWT token
- Admin identities list with register/unregister controls (`register_admin_identity`,
  `unregister_admin_identity`)

---

## Data flow

```
App (auth routing)
 └─ AuthenticatedApp
     ├─ Navbar  (view switching, role-gated links)
     └─ ActiveView routing
         ├─ MySubscriptions  ← message_categories, subscriptions
         ├─ Categories       ← message_categories             [admin]
         ├─ Members          ← visible_accounts, subscriptions [admin]
         └─ Debug            ← admin_identities, connection state [admin]
```

---

## Implementation order

1. **Server**: add `remove_message_category` and `remove_subscription` reducers; add `public` /
   visibility filter to `message_categories` and `subscriptions`.
2. **Regenerate bindings**: `spacetime publish` then `spacetime generate`.
3. **Extend `dioxus.rs`**: add table signals, new hooks, `use_is_admin`.
4. **Create file stubs**: `router.rs`, `components/mod.rs`, `components/navbar.rs`,
   `pages/mod.rs`, and the four page files.
5. **Implement pages** one by one: `subscriptions.rs` first (highest user value), then
   `categories.rs`, `members.rs`, `debug.rs`.
6. **Slim down `main.rs`**: remove `AccountsSection` and `ConnectionStatusCard`, wire
   `Navbar` and `ActiveView` routing.
