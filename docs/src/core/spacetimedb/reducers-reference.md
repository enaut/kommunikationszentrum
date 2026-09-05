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
| `init` | System | `ctx: &ReducerContext` | Lifecycle initializer. Seeds publisher identity into `admin_identities` and initializes schedules for lease expiration and retry requeueing. |
| `identity_connected` | System | `ctx: &ReducerContext` | Triggered on WebSocket connect. Logs client identity. |
| `identity_disconnected` | System | `_ctx: &ReducerContext` | Triggered on WebSocket disconnect. No-op. |
| `register_admin_identity` | Admin | `identity_hex: String` | Grants admin privileges to identity hex string. |
| `unregister_admin_identity` | Admin | `identity_hex: String` | Revokes admin privileges from identity hex string. |
| `create_webhook_token` | Admin | `token_hash: String, label: String, permissions: Vec<String>` | Registers a BLAKE3 hashed bearer token for external webhooks. |
| `revoke_webhook_token` | Admin | `token_hash: String` | Deactivates a bearer webhook token by token hash. |
| `sync_user` | Admin/Webhook | `action: String, user_data: String` | Upserts/deletes user account & syncs category subscriptions from Django. |
| `set_stalwart_config` | Admin | `jmap_url: String, admin_token: String` | Configures or updates Stalwart MTA JMAP REST API endpoint URL and admin bearer token in `stalwart_config`. |

### Category & Subscription Reducers

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `add_message_category` | Admin | `name: String, email_address: String, description: String, visibility: CategoryVisibility` | Creates new mailing list category. |
| `update_message_category` | Admin | `category_id: u64, name: String, description: String, visibility: Option<CategoryVisibility>` | Updates display metadata and visibility of an existing category. |
| `remove_message_category` | Admin | `category_id: u64` | Removes message category and deletes any linked app password. |
| `set_category_topics` | Admin | `category_id: u64, topic_names: Vec<String>` | Replaces a category's topic assignments, creating missing `topics` rows as needed. |
| `rename_topic` | Admin | `topic_id: u64, new_name: String` | Renames an existing topic. |
| `provision_message_category` **`[Procedure]`** | Admin | `name: String, base: String, domain_id: String, description: String, visibility: CategoryVisibility` | Inserts category into DB **and** calls Stalwart JMAP REST API to create mailbox and app password credential. |
| `sync_stalwart_domains` **`[Procedure]`** | Admin/Owner | _(none)_ | Queries Stalwart JMAP REST API (`x:Domain/query`, `x:Domain/get`) and synchronizes domains into the `domains` table. |
| `add_subscription` | User/Admin | `subscriber_account_id: u64, subscriber_email: String, category_id: u64` | Subscribes account to a category (`ManuallySubscribed`). |
| `admin_add_subscription` | Admin | `subscriber_account_id: u64, subscriber_email: String, category_id: u64, status: SubscriptionStatus` | Adds or updates a subscription with an explicit status override (`force = true`). |
| `add_and_subscribe_category` | Admin | `subscriber_account_id: u64, subscriber_email: String, name: String, email_address: String, description: String, visibility: CategoryVisibility` | Idempotently creates category if not present and subscribes the specified account. |
| `remove_subscription` | User/Admin | `subscription_id: u64` | Unsubscribes account (`ManuallyUnsubscribed`). Non-admins cannot remove `RequiredSubscribed`. |
| `ensure_subscription_unsubscribe_token` | Admin/System | `subscription_id: u64` | Ensures an active unsubscribe token exists for the given subscription, reactivating or generating a new one. |

> **Note on List-Unsubscribe:** One-click unsubscription (`LinkUnsubscribed`) is executed via the HTTP endpoint `POST /mailing-list/unsubscribe` rather than a direct client reducer.

### MTA Diagnostics & Log Reducers

MTA hook processing is handled via HTTP handlers (`POST /mta-hook`), documented in [MTA Hook Processing](./http-handlers/mta-hook-processing.md).

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `dump_mta_logs_to_server_logs` | Admin | _(none)_ | Dumps `mta_connection_log` and `mta_message_log` table contents to server log output for debugging. |

### Delivery Pipeline Reducers

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `claim_next_mail_ingress` | Sender Daemon | `instance_id: String` | Claims the next runnable ingress fan-out job by lease and retry time. |
| `increment_mail_ingress_delivery_count` | Sender Daemon | `ingress_id: String, instance_id: String` | Increments the per-ingress success counter after enqueuing deliveries. |
| `increment_mail_ingress_failed_delivery_count` | Sender Daemon | `ingress_id: String, instance_id: String` | Tracks per-ingress failures while preparing deliveries. |
| `complete_mail_ingress` | Sender Daemon | `ingress_id: String, instance_id: String` | Marks the ingress fan-out job complete. |
| `retry_mail_ingress` | Sender Daemon | `ingress_id: String, instance_id: String, error: String` | Re-schedules ingress fan-out retry with exponential backoff. |
| `fail_mail_ingress` | Sender Daemon | `ingress_id: String, instance_id: String, error: String` | Marks ingress as terminally failed. |
| `enqueue_mail_delivery` | Sender Daemon | `ingress_id: String, subscription_id: u64, recipient_email: String, recipient_account_id: Option<u64>, from_header: String, reply_to: String, raw_message: String` | Inserts immutable `MailDeliveryMessage` row and queues recipient delivery in `mail_delivery_pending`. |
| `claim_next_mail_delivery` | Sender Daemon | `instance_id: String` | Claims the next ready delivery from `mail_delivery_pending` and moves it to `mail_delivery_claimed` under worker lease. |
| `mark_mail_delivery_sent` | Sender Daemon | `delivery_id: String, instance_id: String, smtp_status_code: Option<u16>, smtp_response: String` | Deletes claim, logs event, and records terminal state in `mail_delivery_done` (`final_state: Sent`). |
| `schedule_mail_delivery_retry` | Sender Daemon | `delivery_id: String, instance_id: String, error_msg: String, delay_micros: i64` | Deletes claim, logs event, bumps `attempt_count`, and moves record to `mail_delivery_temporary_failed` with `next_attempt_at`. |
| `requeue_temporary_failed_mails` | System (Scheduled) | `_scheduled: RequeueTemporaryFailedMailsSchedule` | Moves expired temporary failure rows back to `mail_delivery_pending`. |
| `cancel_mail_delivery_retry` | Admin | `delivery_id: String` | Removes a delivery from `mail_delivery_temporary_failed` and records cancellation in `mail_delivery_done` (`final_state: Cancelled`). |
| `fail_mail_delivery` | Sender Daemon | `delivery_id: String, instance_id: String, smtp_status_code: Option<u16>, smtp_response: String, error_kind: String` | Deletes claim, logs event, and records terminal failure in `mail_delivery_done` (`final_state: Failed`). |
| `mark_mail_delivery_bounced` | Sender Daemon | `delivery_id: String, instance_id: String, smtp_status_code: Option<u16>, smtp_response: String, error_kind: String` | Deletes claim, logs event, and records bounce in `mail_delivery_done` (`final_state: Bounced`). |
| `expire_stale_delivery_claims` | System (Scheduled) | `_scheduled: ExpireStaleDeliveryClaimsSchedule` | Reclaims expired worker leases for stalled ingress jobs and stalled delivery claims, returning them to pending. |

---

## Provisioning Procedure Architecture

Procedures execute external HTTP side effects outside SpacetimeDB's transactional boundaries before committing database state:

```d2
{{#include reducers-provisioning-procedure.d2}}
```
