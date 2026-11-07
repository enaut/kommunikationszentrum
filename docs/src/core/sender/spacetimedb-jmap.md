# SpacetimeDB-Native Sender

Implementation status for the in-module sender that is replacing the `sender` daemon. The design and full acceptance criteria are in [`plans/sender-spacetimedb-jmap.md`](../../../../plans/sender-spacetimedb-jmap.md).

### Current state

As of 2026-11-07, the server module contains a durable JMAP delivery queue, recurring scheduled dispatcher, mailing-list fan-out, MIME composition, JMAP transport, system-mail routing, retry handling, admin-only delivery/event views, and legacy queue migration/retry APIs. Dioxus bindings for the new APIs are present. The admin UI has not yet been updated to display or manage these records.

The native dispatcher is disabled unless the SpacetimeDB environment value `JMAP_DISPATCH_ENABLED` is set to `true`. Keep it disabled while the legacy sender is processing queues: the system-mail queue is shared, so running both delivery paths can send duplicates. The legacy sender remains part of the project and has not been retired.

### Delivery flow

The scheduled procedure runs every 10 seconds and does nothing unless dispatch is enabled. When enabled, it imports queued system mail into JMAP jobs, fans out at most one ingress message per invocation, and claims up to 10 due delivery rows for one topic/system sending account. Claiming is transactional and uses a token and lease to fence stale completions.

Each delivery is handled individually: compose a recipient-specific RFC 5322 message, upload it, import it into the account's outbox, persist the JMAP Email ID, and submit it. Mailing-list messages use canonical inbound content, verified active subscriber addresses, one recipient per message, a stable delivery correlation header, and unsubscribe headers. System mail uses the same queue and JMAP submission path with system-account credentials.

A successful submission is committed as `Sent` before outbox cleanup is attempted. Transient errors are retried with capped exponential backoff and deterministic jitter; permanent errors remain visible in the delivery table and can be requeued by an administrator. The migration procedure moves only unclaimed legacy pending and temporary-failed deliveries; claimed legacy rows require manual reconciliation.

### Configuration and access

Topic sends use the topic's existing app password and the configured Stalwart JMAP URL. System credentials are typed SpacetimeDB environment values named `SYSTEM_JMAP_USERNAME` and `SYSTEM_JMAP_PASSWORD`; they are not stored in delivery rows or exposed through the admin views. See [module environment secrets](../spacetimedb/module-publishing.md#managing-module-environment-secrets) and the [environment variable reference](../../reference/environment-variables.md).

The delivery and event tables are exposed through views filtered to administrators. The retry reducer also checks administrator identity. Generated client bindings are in `admin/src/module_bindings/`; the status UI and operational controls still need implementation.

### Incomplete and unverified work

- JMAP requests are not batched. The claim size is 10, but imports and submissions are still separate calls per recipient.
- Submission-uncertainty recovery currently queries `EmailSubmission/query`; the planned outbox correlation query as the primary reconciliation path and configurable minimum wait are not implemented.
- Outbox cleanup failures are logged but are not durably retried.
- The current `expire_stale_delivery_claims` reducer does not process JMAP rows. Expired claims are reconsidered by dispatcher selection, but scheduled restart/catch-up behavior has not been tested against a running database.
- Generated bindings exist, but `spacetimedb-cli generate --lang dioxus -p server -o admin/src/module_bindings/` currently exits with a schema-extraction `EOF while parsing a value` error after building the WASM module. Verify regeneration after diagnosing that error.
- No live Stalwart test, production migration, 1,000-recipient throughput test, or controlled system-mail test has been completed.

The implementation has passed `cargo fmt --check --all`, `cargo check -p spacetime-module`, `cargo test -p spacetime-module` (93 tests), a WASM-target `cargo check`, and `spacetimedb-cli build --module-path server`. These checks do not replace live JMAP and scheduler validation.

### Rollout constraint

Do not enable native dispatch until the old sender has been stopped, claimed legacy deliveries have been reconciled, eligible legacy pending work has been migrated, and a controlled test against Stalwart has passed. Keep the legacy daemon and rollback path until production behavior has been verified.
