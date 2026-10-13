# Procedures & Outbound HTTP in SpacetimeDB v2.11

Procedures were introduced in SpacetimeDB 2.0 to solve two critical limitations of reducers:
1. **Returning Values:** Procedures can compute and return values directly to the calling client.
2. **External Side Effects:** Procedures can make outbound HTTP requests to third-party services and APIs.

Because procedures can perform non-deterministic operations (like network I/O), they **do not automatically run in a database transaction**. Database read/write access must be managed explicitly.

---

## 1. Defining a Procedure

Procedures require opting into the `unstable` feature in `Cargo.toml`:
```toml
[dependencies]
spacetimedb = { version = "2.11", features = ["unstable"] }
```

Declare a procedure using `#[spacetimedb::procedure]`:

```rust
use spacetimedb::{procedure, ProcedureContext, SpacetimeType};

#[derive(SpacetimeType)]
pub struct CalculationResult {
    pub sum: u64,
    pub caller_id: String,
}

#[procedure]
pub fn add_numbers(ctx: &mut ProcedureContext, a: u32, b: u32) -> CalculationResult {
    CalculationResult {
        sum: (a as u64) + (b as u64),
        caller_id: ctx.sender().to_string(),
    }
}
```

### Signature Rules:
* First parameter must be `&mut ProcedureContext` (named `ctx`).
* Can take any number of arguments implementing `SpacetimeType`.
* Can return any type implementing `SpacetimeType`. The return value is sent only to the caller.
* If a procedure panics, the panic message is returned to the caller as an error.

---

## 2. Managing Transactions (`with_tx` and `try_with_tx`)

Unlike a `ReducerContext`, a `ProcedureContext` does NOT have a direct `ctx.db` field. To access or modify tables, use transaction closures:

### Infallible Transactions: `ctx.with_tx`
```rust
use spacetimedb::{procedure, ProcedureContext, Table};

#[procedure]
pub fn award_points(ctx: &mut ProcedureContext, user_id: u64, points: u32) {
    ctx.with_tx(|tx| {
        if let Some(mut user) = tx.db.player().id().find(&user_id) {
            user.points += points;
            tx.db.player().id().update(user);
        }
    });
}
```

### Fallible Transactions: `ctx.try_with_tx`
```rust
#[procedure]
pub fn purchase_item(ctx: &mut ProcedureContext, user_id: u64, price: u32) -> Result<String, String> {
    ctx.try_with_tx(|tx| {
        let mut user = tx.db.player().id().find(&user_id)
            .ok_or_else(|| "User not found".to_string())?;

        if user.balance < price {
            return Err("Insufficient balance".to_string());
        }

        user.balance -= price;
        tx.db.player().id().update(user);
        Ok("Purchase successful".to_string())
    })
}
```

### Critical Transaction Rules in Procedures:
1. **Never Call `ctx.http` Inside `with_tx`:** A transaction holds database locks. Performing network I/O inside a transaction causes severe contention or deadlocks. Perform network operations first, then open a brief transaction.
2. **Transaction Retries:** If another transaction commits concurrently and causes a conflict, SpacetimeDB may re-execute the closure passed to `with_tx` up to one retry attempt. **Avoid writing to captured outside variables with interior mutability (`Cell`, `RefCell`) inside the closure.**

---

## 3. Outbound HTTP Client (`ctx.http`)

The `ctx.http` property provides an `HttpClient` to communicate with external APIs.

### Simple `GET` Request
```rust
use spacetimedb::{procedure, ProcedureContext};

#[procedure]
pub fn fetch_weather(ctx: &mut ProcedureContext, city: String) -> Result<String, String> {
    let url = format!("https://api.weather.internal/v1/{city}");
    
    match ctx.http.get(&url) {
        Ok(response) => {
            let status = response.status();
            let body_str = response.into_body().into_string_lossy();
            log::info!("Status: {status}, Body: {body_str}");
            Ok(body_str)
        }
        Err(err) => Err(format!("Network error: {err}")),
    }
}
```

### Complex `POST` Request with Headers and Timeout
```rust
use std::time::Duration;
use spacetimedb::{
    procedure, ProcedureContext, TimeDuration,
    http::{Body, Request, Timeout},
};

#[procedure]
pub fn notify_discord(ctx: &mut ProcedureContext, message: String) -> Result<(), String> {
    let payload = serde_json::json!({ "content": message }).to_string();

    let request = Request::builder()
        .method("POST")
        .uri("https://discord.com/api/webhooks/...")
        .header("Content-Type", "application/json")
        // Set maximum request timeout (e.g. 250ms)
        .extension(Timeout::from(TimeDuration::from(Duration::from_millis(250))))
        .body(Body::from_bytes(payload))
        .map_err(|e| format!("Failed to build request: {e}"))?;

    let response = ctx.http.send(request)
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Discord returned status {}", response.status()));
    }

    Ok(())
}
```

### HTTP Client Features & Limits:
* **Host Timeout Limit:** SpacetimeDB enforces an upper limit of 500 milliseconds for all procedure HTTP calls.
* **Transparent Compression (v2.10.2+):** Outbound HTTP requests automatically request and transparently decompress `gzip` and `Brotli` payloads.
* **Synchronous from Guest Perspective:** The WASM module suspends during host I/O and resumes seamlessly when the HTTP response arrives.

---

## 4. Sleeping in Procedures

Procedures can suspend execution until a designated timestamp using `ctx.sleep_until`:

```rust
use std::time::Duration;
use spacetimedb::{procedure, ProcedureContext};

#[procedure]
pub fn wait_and_check(ctx: &mut ProcedureContext) {
    let resume_at = ctx.timestamp + Duration::from_millis(500);
    ctx.sleep_until(resume_at);
    log::info!("Resumed execution at {}", ctx.timestamp);
}
```
Execution is suspended on the host, freeing executor resources during the wait.
