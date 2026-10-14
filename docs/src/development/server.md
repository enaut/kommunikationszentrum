# Server Module — Developer Guide

This guide covers everything you need to know to develop and extend the server module.

---

## Prerequisites

- Rust toolchain (stable) with `wasm32-unknown-unknown` target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- SpacetimeDB CLI (`spacetime`):
  ```bash
  cargo install spacetime
  ```
- A running SpacetimeDB instance (local or remote):
  ```bash
  spacetime start
  ```

---

## Project Structure

```
server/
├── .cargo/
│   └── config.toml        # Sets default target to wasm32-unknown-unknown
├── Cargo.toml             # crate-type = ["cdylib"], spacetimedb = "2.6"
└── src/
    ├── lib.rs             # Module entry-point, lifecycle reducers
    ├── account.rs         # Account, AdminIdentity, WebhookToken tables + reducers
    ├── topics.rs          # MessageTopic, Category, Subscription, UnsubscribeToken + reducers
    ├── mta.rs             # MTA tables + stage handlers
    ├── delivery.rs        # MailIngress, MailDelivery, MailDeliveryEvent + pipeline reducers
    └── http_handlers.rs   # HTTP router and endpoint handlers
```

The `lib.rs` crate root declares all sub-modules and the three lifecycle reducers (`init`,
`identity_connected`, `identity_disconnected`).

---

## Build

The server module compiles configuration from a TOML profile in `profiles/` (see [Configuration Profiles](../reference/configuration.md)).

```bash
cd server

# Standard build with specific profile (defaults to profiles/example.toml)
APP_PROFILE=my-org cargo build --target wasm32-unknown-unknown
```

The SpacetimeDB CLI handles the WASM build and packaging automatically during `spacetime publish`.

---

## Publish for Development

```bash
cd server

# Publish to local SpacetimeDB with target profile
APP_PROFILE=my-org spacetime publish --server http://localhost:3000 kommunikation

# Watch logs
spacetime logs kommunikation --follow
```

After each code change, re-run `spacetime publish` to update the live module.

---

## Code Organisation Conventions

### Adding a New Table

1. Declare the struct in the appropriate module file with `#[spacetimedb::table(...)]`.
2. Add necessary indexes (`#[index(btree)]`, `#[unique]`, `#[primary_key]`).
3. If the table holds sensitive data, add a `#[spacetimedb::client_visibility_filter]` or a
   `#[spacetimedb::view]` to control what clients see.
4. Export the accessor type if needed by other modules via `pub use`.

### Adding a New Reducer

1. Annotate with `#[spacetimedb::reducer]`.
2. Always check authorization as the first step:
   ```rust
   if !is_admin_user(ctx) {
       return Err("Unauthorized: ...".into());
   }
   ```
3. Return `Result<(), String>` so errors surface cleanly to callers.
4. Log meaningful events with `log::info!` / `log::warn!` / `log::error!`.

### Adding a New HTTP Handler

1. Annotate the function with `#[spacetimedb::http::handler]`.
2. Register it in the `router()` function in `http_handlers.rs`.
3. Use `token_has_permission(ctx, &token, "permission-name")` for authentication.
4. Run all database writes inside `ctx.with_tx(|tx| { … })` for transactional consistency.
5. Return JSON responses using the shared `json_response(status, value)` helper.

---

## Visibility & Privacy Patterns

### Client Visibility Filter (row-level)

Use when a table's rows must be filtered before being sent to any subscriber:

```rust
#[spacetimedb::client_visibility_filter]
pub const ACCOUNT_VISIBILITY: Filter =
    Filter::Sql("SELECT * FROM account WHERE identity = :sender");
```

### View (computed projection)

Use when different callers should see different subsets of the same table:

```rust
#[spacetimedb::view(accessor = visible_accounts, public)]
pub fn visible_accounts(ctx: &ViewContext) -> Vec<Account> {
    let sender = ctx.sender();
    let is_admin = ctx.db.admin_identities().identity().find(&sender).is_some();
    if is_admin {
        ctx.db.account().last_synced().filter(Timestamp::UNIX_EPOCH..).collect()
    } else {
        ctx.db.account().identity().find(&sender).into_iter().collect()
    }
}
```

Clients subscribe to the view name (e.g. `visible_accounts`), not the raw table name.

---

## Delivery Pipeline — Developer Notes

The two-phase delivery pipeline (`mail_ingress` → `mail_deliveries`) is designed for
**at-least-once delivery** with an external sender daemon:

### Claim Protocol

The claim/complete loop is intentionally simple:

```
daemon:  call claim_next_mail_ingress()
         ← SpacetimeDB subscription push: ingress row with claim_owner == own identity
         fan out to deliveries via enqueue_mail_delivery()
         call complete_mail_ingress(ingress_id, delivery_count, failed_count)
```

The lease expiry (`claim_expires_at`) prevents stale claims from blocking progress if the
daemon crashes. Any daemon instance can re-claim after the lease expires.

### Idempotency

- `upsert_mail_ingress` checks for an existing `id` before inserting.
- `upsert_mail_delivery` will not overwrite a terminal delivery (`sent`, `failed`, `bounced`).
- Retry state transitions are guarded by claim owner checks to prevent concurrent updates.

### Adding a New Delivery State

1. Add a `pub const MAIL_DELIVERY_<STATE>: &str = "<state>";` constant in `delivery.rs`.
2. Update `claimable_delivery()` if the new state is claimable.
3. Add the corresponding reducer.
4. Update the sender daemon to handle the new state.

---

## Testing

### Unit Tests (host-side)

Run host-side unit tests using cargo:

```bash
# Run tests with the default example profile
cargo test -p spacetime-module

# Run tests with a specific profile
APP_PROFILE=solawis cargo test -p spacetime-module
```

Integration test scripts are also available:

```bash
cd docs/testscripts
./test_mta_hook.sh
```

### Manual Testing via CLI

```bash
# Add a test topic
spacetime call kommunikation add_message_topic \
  "Test List" "test@example.org" "Test topic"

# Add a test subscription
spacetime call kommunikation add_subscription 42 "user@example.org" 1

# Dump MTA logs
spacetime call kommunikation dump_mta_logs_to_server_logs
spacetime logs kommunikation | grep "MTA"
```

### Inspecting the Database

```bash
# All tables
spacetime sql kommunikation "SELECT * FROM mail_ingress"
spacetime sql kommunikation "SELECT * FROM mail_delivery_pending"
spacetime sql kommunikation "SELECT * FROM mail_delivery_events LIMIT 10"
```

---

## Common Pitfalls

### IP Redaction
Many log entries store `"[REDACTED]"` as the `client_ip` for privacy. This is intentional.
Do not log raw IPs in production unless required for a specific compliance reason.

### Body Size Limit
Messages larger than 2,000,000 bytes addressed to an active mailing-list topic are not fanned out
or archived for subscribers. The DATA handler queues a bilingual system mail to a valid sender
address explaining the limit and records the oversized-message outcome. Messages at or below the
limit retain their complete MIME body for fan-out.

### `visible_*` Views vs Raw Tables
Clients **must** subscribe to views (e.g. `visible_accounts`), not raw table names. Raw tables
either have a client visibility filter that limits rows, or are not declared `public`.

### Configuration Profiles & Stalwart Configuration
Frontend URLs, OIDC issuer URLs, and localized brand names are compiled into the server module at build time from the selected profile in `profiles/` via `server/build.rs`. See [Configuration Profiles](../reference/configuration.md).
Stalwart credentials (`STALWART_JMAP_URL` and `STALWART_ADMIN_TOKEN`) can be set dynamically at runtime via the `set_stalwart_config` reducer (stored in the `stalwart_config` database table) or passed as runtime environment variables fallback (`std::env::var`).

### Subscription Check in DATA Stage
External senders (not present in `account`) are always rejected at the DATA stage, even if the
recipient topic exists. Only known accounts with active subscriptions (or admins) can post.

---

## Useful `spacetime` Commands

```bash
# Publish module
spacetime publish --server http://localhost:3000 kommunikation

# Tail logs
spacetime logs kommunikation --follow

# Call a reducer
spacetime call kommunikation <reducer_name> [args...]

# Run SQL query
spacetime sql kommunikation "<query>"

# List registered modules
spacetime list
```
