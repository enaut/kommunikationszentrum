# Webhooks (Module HTTP Handlers)

This section documents the module-side HTTP routes provided by the Kommunikationszentrum SpacetimeDB module. The module exposes a small number of HTTP handlers that receive external webhooks (MTA hooks and user synchronization requests), validate them, and perform the corresponding database operations inside transactional contexts.

Key points

- HTTP handlers are implemented inside the SpacetimeDB module using the `#[spacetimedb::http::handler]` macro and registered with a `#[spacetimedb::http::router]` function.
- All module routes are mounted under the host path: `/v1/database/:name_or_identity/route/{*path}`. For this project the defaults are:
  - MTA hooks: `/v1/database/kommunikation/route/mta-hook`
  - User sync: `/v1/database/kommunikation/route/user-sync`
- Handlers run inside the module's WASM runtime and may open transactions via `ctx.with_tx(...)` to access the database safely and atomically.
- External callers must authenticate using bearer tokens. Tokens are created and managed from inside the module (reducers) and carry fine-grained permissions.

Security model

- The host intentionally delegates `Authorization` handling to module handlers. As a result, `HandlerContext` does not expose a `sender()` identity; handlers must verify authorization themselves.
- The module stores only a cryptographic hash of bearer tokens (BLAKE3) in the `webhook_tokens` table. Tokens have a `permissions: Vec<String>` column used to restrict which routes a token may call (for example `"mta-hook"` or `"sync-user"`).
- Management of tokens (create/revoke) is implemented as admin-only reducers.

Developer notes

- The module code lives in `server/src/` and defines the HTTP handlers in `server/src/http_handlers.rs`.
- The MTA processing logic is implemented in `server/src/mta.rs`. The handler calls the same stage-processing helpers inside `ctx.with_tx(...)` so behaviour is identical whether the logic is invoked from an internal reducer or from an external HTTP request.
- Build and publish the module using the `spacetime` CLI (do not use `cargo build`):
  - `spacetime build -p server`
  - `spacetime publish --project-path server kommunikation`

Testing

- Test scripts are provided in `docs/testscripts/` and are already configured to POST to the module routes and include the `Authorization: Bearer <token>` header. See `test-mta-hooks.sh` and `test-user-sync.sh`.

If you need a short walkthrough to create a token and run a test, see the API Endpoints and User Synchronization pages in this chapter.
