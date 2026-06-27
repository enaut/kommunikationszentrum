# SpacetimeDB Server Module

The `server` crate is the heart of the Kommunikationszentrum. It is compiled to a **WebAssembly (WASM)** module and published to SpacetimeDB, where it runs as a fully reactive database with embedded business logic.

## Purpose

The server module combines database schema, business logic, and HTTP route handlers into a single deployable unit. It:

- Receives and validates **incoming email** from the Stalwart MTA via HTTP hooks
- Manages **user accounts** synchronized from Django solawispielplatz
- Controls **mailing list subscriptions** and enforces send permissions
- Drives the **delivery pipeline** that fans a received message out to individual subscriber inboxes
- Exposes **real-time data** to the Admin UI and the sender daemon via SpacetimeDB WebSocket subscriptions

## Technology

| Concern | Technology |
|---|---|
| Runtime | SpacetimeDB 2.6 (WASM) |
| Language | Rust (crate-type `cdylib`) |
| MTA hook types | `stalwart_mta_hook_types` |
| Serialization | `serde` + `serde_json` |
| Token hashing | `blake3` |

## Source File Map

```
server/src/
├── lib.rs            Crate entry-point: lifecycle reducers (init, connected, disconnected)
├── account.rs        Account & identity tables, admin management, user-sync logic
├── mailing.rs        Message categories, subscriptions, unsubscribe tokens, provision procedure
├── mta.rs            MTA tables (connection log, message log, received_message, blocked_ips)
│                     and stage handlers: connect → ehlo → mail → rcpt → data → auth
├── delivery.rs       Delivery pipeline tables (mail_ingress, mail_deliveries, mail_delivery_events)
│                     and claim/complete/retry reducers
└── http_handlers.rs  HTTP router and handlers: /mta-hook, /user-sync, /mailing-list/unsubscribe
```

## Module Sections

| Section | File(s) | Description |
|---|---|---|
| [Tables Reference](./tables-reference.md) | all | Every table, its fields and indexes |
| [Reducers Reference](./reducers-reference.md) | all | Every reducer, procedure and view |
| [HTTP Handlers](./http-handlers.md) | `http_handlers.rs` | Webhook endpoints and authentication |
| [Event & Trigger Flow](./event-flow.md) | all | End-to-end flow diagrams for email and user sync |

## Key Design Decisions

### Visibility via Views

Raw tables such as `account`, `subscriptions`, and `received_message` are restricted through
SpacetimeDB **client visibility filters** and **views**. Regular users only see their own rows;
admins see everything.

```
account              → ACCOUNT_VISIBILITY filter (own rows only)
visible_accounts     → view: all rows for admins, own row for others
visible_messages     → view: all received_message for admins, subscribed categories for users
visible_subscriptions → view: all for admins, own for users
visible_webhook_tokens → view: empty for non-admins
```

### Two-Phase Delivery Pipeline

Email does not go directly from the MTA hook to subscriber inboxes. Instead, the DATA stage
creates a `MailIngress` record that the external **sender daemon** picks up via a claim/complete
protocol, fans out to individual `MailDelivery` rows, and submits over SMTP.

See [Event & Trigger Flow](./event-flow.md) for the full diagram.

### Webhook Token Security

External callers (MTA, Django) authenticate with **bearer tokens**. The plaintext token is
never stored; only its BLAKE3 hash is kept in `webhook_tokens`. Permissions are a `Vec<String>`
so multiple scopes can be granted to a single token.

Available permission strings:
- `mta-hook` — call the `/mta-hook` endpoint
- `sync-user` — call the `/user-sync` endpoint
