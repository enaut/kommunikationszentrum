# Reducers & Procedures Reference

Reducers are atomic, transactional WebAssembly functions in SpacetimeDB that execute database state transitions. Procedures extend reducers with side-effecting external I/O capabilities (such as Stalwart MTA JMAP REST calls).

## Caller-to-Reducer Architecture

```d2
{{#include reducers-caller-architecture.d2}}
```

---

## Delivery Pipeline State Machine & Reducers

The async delivery pipeline transitions ingress records and individual recipient delivery records via worker leases.

```d2
{{#include reducers-delivery-pipeline-state.d2}}
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
| `add_message_category` | Admin | `name: String, email_address: String, description: String, visibility: CategoryVisibility` | Creates new mailing list category. |
| `update_message_category` | Admin | `id: u64, name: String, description: String, visibility: Option<CategoryVisibility>` | Updates display metadata and visibility of a category. |
| `set_message_category_active` | Admin | `id: u64, active: bool` | Toggles category active status. |
| `delete_message_category` | Admin | `id: u64` | Removes message category by ID. |
| `set_category_topics` | Admin | `category_id: u64, topic_names: Vec<String>` | Replaces a category's topic assignments, creating missing `topics` rows as needed. |
| `rename_topic` | Admin | `topic_id: u64, new_name: String` | Renames an existing topic. |
| `provision_message_category` **`[Procedure]`** | Admin | `name: String, base: String, domain_id: String, description: String, visibility: CategoryVisibility` | Inserts category into DB **and** calls Stalwart JMAP REST API to create mailbox. |
| `sync_stalwart_domains` **`[Procedure]`** | Admin/Owner | _(none)_ | Queries Stalwart JMAP REST API (`x:Domain/query`, `x:Domain/get`) and synchronizes domains into the `domains` table. |
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
| `claim_next_mail_ingress` | Sender Daemon | `instance_id: String` | Claims the next runnable ingress fan-out job by lease and retry time. |
| `increment_mail_ingress_delivery_count` | Sender Daemon | `ingress_id: String, instance_id: String` | Increments the per-ingress success counter after enqueuing deliveries. |
| `increment_mail_ingress_failed_delivery_count` | Sender Daemon | `ingress_id: String, instance_id: String` | Tracks per-ingress failures while preparing deliveries. |
| `complete_mail_ingress` | Sender Daemon | `ingress_id: String, instance_id: String` | Marks the ingress fan-out job complete. |
| `retry_mail_ingress` | Sender Daemon | `ingress_id: String, instance_id: String, error: String` | Re-schedules ingress fan-out retry if a transient failure occurs. |
| `fail_mail_ingress` | Sender Daemon | `ingress_id: String, instance_id: String, error: String` | Marks ingress as terminally failed. |
| `enqueue_mail_delivery` | Sender Daemon | `ingress_id: String, subscription_id: u64, recipient_email: String, ...` | Adds a subscriber delivery to `mail_delivery_pending`. |
| `claim_next_mail_delivery` | Sender Daemon | `instance_id: String` | Claims the next ready delivery using a simple FIFO order. |
| `mark_mail_delivery_sent` | Sender Daemon | `delivery_id: String, instance_id: String, smtp_status_code: Option<u16>, smtp_response: String` | Marks a delivery as sent and records it in `mail_delivery_done`. |
| `schedule_mail_delivery_retry` | Sender Daemon | `delivery_id: String, error_msg: String, delay_micros: i64` | Deletes the active claim, bumps `attempt_count`, and inserts the record into `mail_delivery_temporary_failed`. |
| `requeue_temporary_failed_mails` | System | `_ctx: &ReducerContext` | Moves all expired temporary failures back to `mail_delivery_pending`. |
| `cancel_mail_delivery_retry` | Admin | `delivery_id: String` | Removes a temporarily failed delivery from the retry queue and records the cancellation as final. |
| `fail_mail_delivery` | Sender Daemon | `delivery_id: String, instance_id: String, smtp_status_code: Option<u16>, smtp_response: String, error_kind: String` | Marks a delivery as permanently failed. |
| `mark_mail_delivery_bounced` | Sender Daemon | `delivery_id: String, instance_id: String, smtp_status_code: Option<u16>, smtp_response: String, error_kind: String` | Records a bounce and stores the final state. |
| `expire_stale_delivery_claims` | System | `_scheduled: ExpireStaleDeliveryClaimsSchedule` | Reclaims expired worker leases and requeues stalled deliveries. |

---

## Provisioning Procedure Architecture

Procedures execute external HTTP side effects outside SpacetimeDB's transactional boundaries before committing database state:

```d2
{{#include reducers-provisioning-procedure.d2}}
```
