# Outbound Delivery

Outbound mail is processed by the scheduled SpacetimeDB module in `server`. There is no separate sender worker. The module composes recipient-specific messages and submits them to Stalwart through JMAP.

### Dispatch

The module creates a recurring dispatch schedule. A scheduled invocation exits without sending when `JMAP_DISPATCH_ENABLED` is not `true`. System-mail credentials are read from typed SpacetimeDB environment values `SYSTEM_JMAP_USERNAME` and `SYSTEM_JMAP_PASSWORD`; topic messages use the topic's stored app password.

When enabled, dispatch processes queued system mail and accepted ingress messages, creates durable per-recipient JMAP delivery rows, claims due work, and performs JMAP upload, import, and submission requests outside database transactions. Delivery state and audit events are stored in SpacetimeDB. Transient or uncertain outcomes are retried; permanent failures are visible in the admin status page and can be manually retried by an administrator.

The Admin **Status** page displays JMAP delivery counts, per-ingress progress, delivery history, and recent events. It subscribes to administrator-filtered delivery and event views.

### Legacy queue records

The module retains the legacy SMTP queue tables and the `migrate_pending_legacy_deliveries` procedure so existing pending and temporary-failure rows can be migrated. The procedure does not move claimed rows because Stalwart may already have accepted them. Preserve those rows until their outcomes have been reconciled; schema publication must not be treated as a data reset.

### Operational state

The scheduled JMAP path remains gated by `JMAP_DISPATCH_ENABLED`. Before enabling dispatch in a deployed database, verify credentials, scheduled execution, pending queue state, and claimed-row reconciliation; schema publication does not migrate or reset existing production queue rows.
