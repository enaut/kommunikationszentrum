# Inbound HTTP Handlers & Routers in SpacetimeDB v2.11

SpacetimeDB modules can expose direct HTTP endpoints (such as webhooks, REST endpoints, and integration callbacks) without needing an external reverse proxy or web framework.

---

## 1. Defining HTTP Handlers

An HTTP handler is declared with `#[spacetimedb::http::handler]`:

```rust
use spacetimedb::http::{handler, Body, HandlerContext, Request, Response};

#[handler]
fn handle_ping(_ctx: &mut HandlerContext, _req: Request) -> Response {
    Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(Body::from_bytes(b"{\"message\":\"pong\"}".as_slice()))
        .unwrap()
}
```

### Handler Function Rules:
* Accepts exactly two arguments:
  1. `ctx: &mut HandlerContext`
  2. `req: http::Request<spacetimedb::http::Body>`
* Returns `http::Response<spacetimedb::http::Body>`.
* The `#[handler]` macro registers the function and generates a `Handler` reference token. User code does not call the handler function directly; it is passed to the database router.

---

## 2. Registering Routes with `#[router]`

Every module exposing HTTP endpoints must define a single parameterless router function returning `Router` annotated with `#[spacetimedb::http::router]`:

```rust
use spacetimedb::http::{router, Router};

#[router]
fn http_router() -> Router {
    Router::new()
        .get("/health", handle_health)
        .post("/api/webhook", handle_webhook)
        .post("/api/v1/stripe", handle_stripe)
        .nest("/admin", admin_router())
}

fn admin_router() -> Router {
    Router::new()
        .get("/stats", handle_admin_stats)
}
```

### HTTP Router Methods:
* `.get(path, handler)`
* `.post(path, handler)`
* `.put(path, handler)`
* `.delete(path, handler)`
* `.patch(path, handler)`
* `.head(path, handler)`
* `.options(path, handler)`
* `.any(path, handler)` (matches all HTTP methods for the specified path)
* `.nest(path, sub_router)` (mounts a sub-router below a prefix)
* `.merge(other_router)` (combines two routers)

### Strict Path Routing Rules:
1. Path must begin with `/` (except the root empty string `""`).
2. Only ASCII lowercase letters, numbers, and `-_~/` are permitted.
3. Strict routing: Trailing slashes are significant (`/api/data` and `/api/data/` are distinct routes).

---

## 3. HandlerContext Capabilities & Transactions

`HandlerContext` provides access to the invocation timestamp and database transactions:

```rust
#[handler]
fn process_incoming_order(ctx: &mut HandlerContext, req: Request) -> Response {
    // Authenticate request before opening transaction (see section 4)
    if !authenticate_request(ctx, &req) {
        return Response::builder()
            .status(401)
            .body(Body::from_bytes(b"{\"error\":\"Unauthorized\"}".as_slice()))
            .unwrap();
    }

    let body_bytes = req.into_body().into_bytes();
    let order: OrderPayload = match serde_json::from_slice(&body_bytes) {
        Ok(data) => data,
        Err(_) => {
            return Response::builder()
                .status(400)
                .body(Body::from_bytes(b"{\"error\":\"Invalid JSON\"}".as_slice()))
                .unwrap();
        }
    };

    // Open a database transaction
    let order_id = ctx.with_tx(|tx| {
        tx.db.orders().insert(Order {
            id: 0,
            item: order.item,
            amount: order.amount,
            created_at: tx.timestamp,
        }).id
    });

    Response::builder()
        .status(201)
        .header("content-type", "application/json")
        .body(Body::from_bytes(format!("{{\"order_id\":{order_id}}}")))
        .unwrap()
}
```

---

## 4. Production Webhook Security Pattern

HTTP endpoints are publicly exposed on the SpacetimeDB node. Always authenticate incoming webhooks via bearer tokens or HMAC signature validation:

```rust
use spacetimedb::http::{handler, Body, HandlerContext, Request, Response};
use serde_json::json;

fn authenticate_request(ctx: &mut HandlerContext, req: &Request) -> bool {
    let token = match req.headers().get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ")) {
            Some(t) => t.trim(),
            None => return false,
        };

    // Query token hash from database table
    let hash = hex::encode(blake3::hash(token.as_bytes()).as_bytes());
    ctx.with_tx(|tx| {
        tx.db.webhook_tokens()
            .token_hash()
            .find(&hash)
            .map_or(false, |t| t.is_active)
    })
}

#[handler]
fn secure_webhook(ctx: &mut HandlerContext, req: Request) -> Response {
    if !authenticate_request(ctx, &req) {
        return Response::builder()
            .status(401)
            .header("content-type", "application/json")
            .body(Body::from_bytes(b"{\"error\":\"Unauthorized\"}".as_slice()))
            .unwrap();
    }

    // Process authenticated webhook...
    Response::builder().status(200).body(Body::empty()).unwrap()
}
```
