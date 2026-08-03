# Reducers & Procedures Reference

Reducers are atomic, transactional WebAssembly functions in SpacetimeDB that execute database state transitions. Procedures extend reducers with side-effecting external I/O capabilities (such as Stalwart MTA JMAP REST calls).

## Caller-to-Reducer Architecture

```d2
direction: down

"Client & Integration Callers": {
  AdminUI: {
    shape: person
    label: "Admin UI Client"
  }
  UserClient: {
    shape: person
    label: "Member Client"
  }
  MTAHook: {
    shape: cloud
    label: "Stalwart MTA Hook (HTTP)"
  }
  DjangoWebhook: {
    shape: cloud
    label: "Django Webhook (HTTP)"
  }
  SenderDaemon: {
    shape: rectangle
    label: "Sender Daemon Worker"
  }
}

"SpacetimeDB Reducers & Procedures": {
  "Auth & Identity": {
    register_admin_identity
    unregister_admin_identity
    generate_webhook_token
    revoke_webhook_token
    sync_user
  }

  "Category Management": {
    add_message_category
    update_message_category
    set_message_category_active
    delete_message_category
    provision_message_category: "Procedure (JMAP REST Call)"
  }

  "Subscription Flow": {
    subscribe
    unsubscribe
    unsubscribe_by_token: "List-Unsubscribe POST"
    admin_set_subscription_status
  }

  "MTA Hook & Security": {
    handle_mta_hook
    block_ip
  }

  "Delivery Pipeline": {
    ingress_reducers: "claim / heartbeat / complete ingress"
    delivery_reducers: "fan-out / claim / record success & failure"
  }
}

"Database Tables": {
  account
  admin_identities
  webhook_tokens
  message_categories
  subscriptions
  subscription_unsubscribe_tokens
  mta_connection_log
  mta_message_log
  received_message
  blocked_ips
  mail_ingress
  mail_deliveries
  mail_delivery_events
}

# Invocations & Mutations
"Client & Integration Callers".AdminUI -> "SpacetimeDB Reducers & Procedures"."Auth & Identity": "Admin actions"
"Client & Integration Callers".AdminUI -> "SpacetimeDB Reducers & Procedures"."Category Management": "Manage categories"
"Client & Integration Callers".AdminUI -> "SpacetimeDB Reducers & Procedures"."MTA Hook & Security".block_ip: "IP block"

"Client & Integration Callers".UserClient -> "SpacetimeDB Reducers & Procedures"."Subscription Flow".subscribe: "User opt-in"

"Client & Integration Callers".DjangoWebhook -> "SpacetimeDB Reducers & Procedures"."Auth & Identity".sync_user: "User/VP Sync"
"Client & Integration Callers".MTAHook -> "SpacetimeDB Reducers & Procedures"."MTA Hook & Security".handle_mta_hook: "Hook Events"

"Client & Integration Callers".SenderDaemon -> "SpacetimeDB Reducers & Procedures"."Delivery Pipeline": "Lease & Execute"

"SpacetimeDB Reducers & Procedures" -> "Database Tables": "Transactional Mutate"
```

---

## Delivery Pipeline State Machine & Reducers

The async delivery pipeline transitions ingress records and individual recipient delivery records via worker leases.

```d2
direction: right

IngressLifecycle: {
  pending -> processing: "claim_pending_ingress()"
  processing -> processing: "heartbeat_ingress_claim()"
  processing -> completed: "complete_ingress()"
}

DeliveryLifecycle: {
  queued -> sending: "claim_pending_deliveries()"
  sending -> sending: "heartbeat_delivery_claim()"
  sending -> sent: "record_delivery_success()"
  sending -> retry_scheduled: "record_delivery_failure() [transient]"
  sending -> failed: "record_delivery_failure() [max retries]"
  sending -> bounced: "record_delivery_failure() [permanent]"
  retry_scheduled -> queued: "next_attempt_at reached"
}
```

---

## Reducer & Procedure Reference

### Auth & System Reducers

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `init` | System | `_ctx: &ReducerContext` | Lifecycle initializer. Seeds publisher identity into `admin_identities`. |
| `identity_connected` | System | `ctx: &ReducerContext` | Triggered on WebSocket connect. Logs client identity. |
| `identity_disconnected` | System | `_ctx: &ReducerContext` | Triggered on WebSocket disconnect. No-op. |
| `register_admin_identity` | Admin | `identity_hex: String` | Grants admin privileges to identity hex string. |
| `unregister_admin_identity` | Admin | `identity_hex: String` | Revokes admin privileges from identity. |
| `generate_webhook_token` | Admin | `label: String, permissions: Vec<String>` | Generates a hashed bearer token for external webhooks. |
| `revoke_webhook_token` | Admin | `id: u64` | Deactivates a bearer webhook token by ID. |
| `sync_user` | Admin/Webhook | `action: String, user_data: String` | Upserts/deletes user account & syncs category subscriptions from Django. |

### Category & Subscription Reducers

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `add_message_category` | Admin | `name: String, email_address: String, description: String` | Creates new mailing list category. |
| `update_message_category` | Admin | `id: u64, name: String, description: String` | Updates display metadata of a category. |
| `set_message_category_active` | Admin | `id: u64, active: bool` | Toggles category active status. |
| `delete_message_category` | Admin | `id: u64` | Removes message category by ID. |
| `provision_message_category` **`[Procedure]`** | Admin | `name: String, email_address: String, description: String` | Inserts category into DB **and** calls Stalwart JMAP REST API to create mailbox. |
| `subscribe` | User/Admin | `subscriber_account_id: u64, subscriber_email: String, category_id: u64` | Subscribes account to a category (`ManuallySubscribed`). |
| `unsubscribe` | User/Admin | `subscription_id: u64` | Unsubscribes account (`ManuallyUnsubscribed`). |
| `unsubscribe_by_token` | Public | `token: String` | Processes one-click `List-Unsubscribe-Post` HTTP link (`LinkUnsubscribed`). |
| `admin_set_subscription_status` | Admin | `subscription_id: u64, status: SubscriptionStatus` | Overrides subscription status directly. |

### MTA Hook & Security Reducers

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `handle_mta_hook` | MTA Webhook | `hook_data: String` | Parses Stalwart hook JSON (CONNECT, EHLO, MAIL, RCPT, DATA, AUTH), validates IP/subscriptions, logs audit events, creates `mail_ingress`. |
| `block_ip` | Admin | `ip: String, reason: String` | Blacklists IP address in `blocked_ips`. |

### Delivery Pipeline Reducers

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `create_mail_ingress` | Module Internal | `ingress: MailIngress` | Inserts new `mail_ingress` record in `pending` state. |
| `claim_pending_ingress` | Sender Daemon | `worker_identity: Identity` | Claims pending ingress lease (`claim_owner`, `claim_expires_at`). |
| `heartbeat_ingress_claim` | Sender Daemon | `ingress_id: String` | Renews 10-minute ingress lease. |
| `complete_ingress` | Sender Daemon | `ingress_id: String` | Marks fan-out processing complete. |
| `create_mail_deliveries` | Sender Daemon | `ingress_id: String, deliveries: Vec<MailDelivery>` | Bulk-inserts subscriber delivery records. |
| `claim_pending_deliveries` | Sender Daemon | `worker_identity: Identity, limit: u32` | Claims pending delivery leases (`claim_owner`, `claim_expires_at`). |
| `heartbeat_delivery_claim` | Sender Daemon | `delivery_ids: Vec<String>` | Renews 5-minute delivery worker leases. |
| `record_delivery_attempt` | Sender Daemon | `delivery_id: String, attempt_no: u32, ...` | Appends audit event record to `mail_delivery_events`. |
| `record_delivery_success` | Sender Daemon | `delivery_id: String, smtp_code: u16, smtp_response: String` | Transitions delivery to `sent` state. |
| `record_delivery_failure` | Sender Daemon | `delivery_id: String, error_kind: String, details: String, is_permanent: bool` | Handles back-off reschedule (`retry_scheduled`), max attempts (`failed`), or bounce (`bounced`). |

---

## Provisioning Procedure Architecture

Procedures execute external HTTP side effects outside SpacetimeDB's transactional boundaries before committing database state:

```d2
direction: right

Client -> SpacetimeDB: "Call provision_message_category()"

SpacetimeDB: {
  DB_Check: "Verify Admin Identity & Unique Email"
  Stalwart_HTTP: {
    shape: cloud
    label: "Stalwart JMAP API\nPOST /jmap/session"
  }
  DB_Insert: "Insert message_categories row"
  DB_Check -> Stalwart_HTTP: "Create Mailbox"
  Stalwart_HTTP -> DB_Insert: "Mailbox Created (200 OK)"
}

SpacetimeDB -> Client: "Result: Ok(category_id)"
```
