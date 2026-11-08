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
| `create_webhook_token` | Admin | `token_hash: String, label: String, permissions: Vec<String>` | Registers a BLAKE3 hashed bearer token for external webhooks (see [Managing Webhook Tokens](./module-publishing.md#managing-webhook-tokens)). |
| `revoke_webhook_token` | Admin | `token_hash: String` | Deactivates a bearer webhook token by token hash. |
| `sync_user` | Admin/Webhook | `action: String, user_data: String` | Upserts/deletes user account & syncs topic subscriptions from Django. |
| `set_stalwart_config` | Admin | `jmap_url: String, admin_token: String` | Configures or updates Stalwart MTA JMAP REST API endpoint URL and admin bearer token in `stalwart_config`. |

### Account & Multi-Email Reducers

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `admin_add_account_email` | Admin | `account_id: u64, email: String` | Directly adds an email address to a member account (pre-verified with `source = Native`). Enforces uniqueness per account. |
| `user_request_email_verification` | User | `email: String` | Initiates email addition for caller's account. Generates a token and queues a verification email in `system_mail_pending`. |
| `user_verify_email` | User/Public | `token: String` | Validates verification token, inserts `AccountEmail` marked verified, and cleans up token. |
| `remove_account_email` | User/Admin | `account_email_id: u64` | Removes an email from the account. Cascades deletion to associated subscriptions and unsubscribe tokens. Cannot delete primary email. |

### Topic, Category & Subscription Reducers

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `add_message_topic` | Admin | `name: String, email_address: String, description: String, visibility: TopicVisibility` | Creates new message topic (mailing list). |
| `update_message_topic` | Admin / Topic Moderator | `topic_id: u64, name: String, description: String, visibility: Option<TopicVisibility>, default_permission: Option<SubscriptionPermission>, clear_provisioning_lock: Option<bool>` | Updates display metadata, permission, and visibility of an existing topic. Only admins can set or remove `Moderate` default permission. |
| `remove_message_topic` | Admin | `topic_id: u64` | Removes message topic and deletes any linked app password. |
| `clear_topic_provisioning_lock` | Admin / Topic Moderator | `topic_id: u64` | Clears provisioning lock on topic. |
| `set_topic_categories` | Admin / Topic Moderator | `topic_id: u64, category_names: Vec<String>` | Replaces a topic's category assignments, creating missing `categories` rows as needed. |
| `rename_category` | Admin | `category_id: u64, new_name: String` | Renames an existing category tag. |
| `provision_message_topic` **`[Procedure]`** | Admin | `name: String, base: String, domain_id: String, description: String, visibility: TopicVisibility` | Inserts topic into DB **and** calls Stalwart JMAP REST API to create mailbox and app password credential. |
| `provision_all_unprovisioned_topics` **`[Procedure]`** | Admin | _(none)_ | Provisions Stalwart mailboxes and app passwords for all existing topics lacking credentials. |
| `sync_stalwart_domains` **`[Procedure]`** | Admin/Owner | _(none)_ | Queries Stalwart JMAP REST API (`x:Domain/query`, `x:Domain/get`) and synchronizes domains into the `domains` table. |
| `add_subscription` | User / Moderator / Admin | `subscriber_account_id: u64, account_email_id: u64, topic_id: u64` | Subscribes account's verified email to a topic (`ManuallySubscribed`). Enforces email verification and private topic access rules. |
| `admin_add_subscription` | Admin / Topic Moderator | `subscriber_account_id: u64, account_email_id: u64, topic_id: u64, status: SubscriptionStatus` | Adds or updates a subscription with an explicitly chosen status. Non-admins can only set `ManuallySubscribed` or `ManuallyUnsubscribed`, and cannot modify `RequiredSubscribed` or moderator subscriptions. |
| `add_and_subscribe_topic` | Admin | `subscriber_account_id: u64, account_email_id: u64, name: String, email_address: String, description: String, visibility: TopicVisibility` | Idempotently creates topic if not present and subscribes the specified account email. |
| `update_subscription_permission` | Admin / Topic Moderator | `subscription_id: u64, permission: SubscriptionPermission` | Updates subscription permission mode (`Read`, `Write`, or `Moderate`). Only Admins can assign or revoke `Moderate`. |
| `remove_subscription` | User / Moderator / Admin | `subscription_id: u64` | Unsubscribes account email (`ManuallyUnsubscribed`). Non-admins and non-moderators can only remove their own; moderators can remove subscriptions on their topics (except peer moderators and `RequiredSubscribed`). Non-admins cannot remove `RequiredSubscribed`. |
| `ensure_subscription_unsubscribe_token` | Admin/System | `subscription_id: u64` | Ensures an active unsubscribe token exists for the given subscription, reactivating or generating a new one. |
| `user_unsubscribe_by_token` | User/Public | `token: String` | Unsubscribes a subscription by its one-click unsubscribe token (`LinkUnsubscribed`). |

> **Note on List-Unsubscribe:** One-click unsubscription (`LinkUnsubscribed`) can be executed via the Dioxus Admin frontend using the `user_unsubscribe_by_token` reducer, or via the HTTP endpoint `/mailing-list/unsubscribe` (which accepts RFC 8058 `POST` requests and redirects browser `GET` requests to the frontend).

### MTA & Mail Retention Reducers

MTA hook processing is handled via HTTP handlers (`POST /mta-hook`), documented in [MTA Hook Processing](./http-handlers/mta-hook-processing.md). Retention of rejected messages is managed by configurable settings and a scheduled purge reducer.

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `set_mail_retention_config` | Admin | `rejected_retention_days: u32` | Updates the retention period (in days, default 30) before rejected messages are permanently purged. |
| `purge_rejected_mails` | Scheduled / Internal | `_timer: PurgeRejectedMailsSchedule` | Scheduled reducer running every 24 hours to purge rejected `received_message` rows older than the retention cutoff and any orphaned `mail_message` records. |

### Native JMAP Delivery

System notifications and accepted mailing-list messages are dispatched by the scheduled procedure `dispatch_jmap_mail`. It is gated by `JMAP_DISPATCH_ENABLED` and performs JMAP I/O outside reducer transactions. Delivery and event state is stored in the native JMAP tables and exposed to administrators through scoped views.

| Function | Visibility | Parameters | Description |
|---|---|---|---|
| `dispatch_jmap_mail` | Scheduled / Internal | Scheduled row | Imports queued system mail, fans out accepted ingress, and dispatches due JMAP deliveries when enabled. |
| `retry_failed_jmap_delivery` | Admin | `delivery_id: String` | Requeues a permanently failed JMAP delivery after an administrator addresses its cause. |
| `migrate_pending_legacy_deliveries` | Admin procedure | none | Moves unclaimed legacy pending and temporary-failure rows into the JMAP queue. Claimed legacy rows are left for explicit reconciliation. |

Legacy SMTP queue tables remain private for migration and reconciliation. The `migrate_pending_legacy_deliveries` procedure moves eligible pending rows into the JMAP queue; claimed rows require explicit outcome reconciliation.

---

## Provisioning Procedure Architecture

Procedures execute external HTTP side effects outside SpacetimeDB's transactional boundaries before committing database state:

```d2
{{#include reducers-provisioning-procedure.d2}}
```
