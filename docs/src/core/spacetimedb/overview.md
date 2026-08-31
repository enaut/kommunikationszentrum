# SpacetimeDB Server

# SpacetimeDB in the System Architecture

In this project, **SpacetimeDB** is utilized as the core component for data storage and primary business logic. It replaces the traditional pattern of maintaining a separate backend server and a standalone relational database.

## What is SpacetimeDB?

SpacetimeDB is a relational **real-time database** that executes application logic directly inside the database engine. Instead of sending SQL queries from an external server, the server-side code is written in Rust and deployed as a WebAssembly (WASM) module running directly within the database. In our architecture, the SpacetimeDB server (Port 3000) acts as the primary single source of truth for subscriptions and email-routing logic.

## Pros and Cons in the Project Context

### Pros

* **Maximum Performance & Minimal Lateness:** Because the application code and data live in the same memory space, network roundtrips between a traditional backend and the database are completely eliminated.
* **No ORM Overhead:** The data structures defined in the Rust code map directly to the database tables. No separate Object-Relational Mapper is required.
* **Out-of-the-Box Real-Time Sync:** Status changes in the database are automatically pushed via WebSockets to the Admin Web UI (Dioxus), eliminating the need for complex polling mechanisms.
* **End-to-End Type Safety:** Utilizing Rust for both the server module and the frontend guarantees strict data consistency across the entire application stack.

### Cons & Challenges

* **Increased Synchronization Effort:** Since user management and the OAuth provider are handled by the external Django backend, manual or scripted data synchronization (`sync_users_to_spacetimedb`) is required to keep user records aligned.
* **Niche Technology:** As a relatively new ecosystem, the community and available tooling are smaller compared to established stacks like PostgreSQL with Node.js or Python. This can make troubleshooting deep edge cases more difficult.
* **WASM Sandbox Constraints:** Because the server module executes inside a WebAssembly environment, direct access to the underlying operating system or certain standard networking libraries is restricted.

## Integration within the Kommunikationszentrum

The database primarily interacts with four components in the ecosystem:
1. **Admin Web UI (Dioxus):** Connects directly via the SpacetimeDB client to display and manipulate configurations in real time.
2. **Django Backend:** Populates SpacetimeDB with relevant user and profile data via management commands.
3. **Stalwart MTA:** Posts incoming email events to SpacetimeDB for processing and routing.
4. **Sender Daemon:** Subscribes to SpacetimeDB views to retrieve messages for delivery to users.


## 5. WebSocket Subscription Model

The Admin UI and sender daemon connect to SpacetimeDB over WebSocket and subscribe to **views**, not raw tables. SpacetimeDB pushes incremental row updates whenever a subscribed view's result set changes.

```d2
{{#include event-flow-websocket-subscription.d2}}
```

**Current view selection by identity:**
- Admin clients receive access to `visible_accounts`, `visible_admin_identities`, `visible_webhook_tokens`, `visible_domains`, `visible_category_app_passwords`, and the full `visible_messages` stream.
- Regular members receive a filtered `visible_accounts` row, their own `visible_subscriptions`, their visible mailing-list metadata, and message rows for subscribed categories only.
- The sender daemon connects as an admin identity and subscribes to outbound worker views such as `sender_mail_ingress`, `sender_mail_delivery_pending`, `sender_mail_delivery_claimed`, `sender_mail_delivery_done`, `sender_mail_delivery_events`, `sender_mail_delivery_temporary_failed`, and `sender_mail_messages`.
