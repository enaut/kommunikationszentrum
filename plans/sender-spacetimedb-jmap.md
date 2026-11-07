# SpacetimeDB-Native Sender with Stalwart JMAP

## Status

Implementation is in progress. The server-side JMAP pipeline, durable delivery schema, scheduled dispatcher, fan-out, composition, migration procedure, admin views, and generated bindings are present in the worktree. The legacy sender remains in place, and native dispatch is disabled unless `JMAP_DISPATCH_ENABLED=true` is set. Do not enable it while the legacy sender is processing the shared queues.

## Implementation state (2026-11-07)

### Done

- Added durable per-recipient JMAP delivery rows and audit events, plus a recurring scheduled dispatcher record.
- Added admin-only delivery/event views, an administrator retry reducer for permanently failed rows, and a migration procedure for unclaimed legacy pending and temporary-failed rows. Claimed legacy rows are deliberately left for manual reconciliation.
- Added scheduled in-module fan-out for one ingress at a time, including active-subscription and verified-email eligibility, recipient deduplication, unsubscribe tokens, and deterministic delivery IDs.
- Added per-recipient MIME composition and JMAP session discovery, upload, import, submission, and retry handling using `ctx.http`.
- Routed queued system mail into the same JMAP delivery table using typed module environment values for system credentials.
- Added generated Dioxus binding files for the JMAP tables and retry/migration APIs. The generation command currently writes these outputs but exits unsuccessfully during schema extraction (`EOF while parsing a value`); verify generation after the extraction failure is diagnosed.
- Added a local `mail-builder` 2.0.1 compatibility patch. The upstream `web-time` dependency brings in `wasm-bindgen`, which SpacetimeDB modules reject. The patch removes that dependency and uses `std::time`; sender messages set explicit Date headers.
- Verified formatting, native Rust checks/tests, WASM-target `cargo check`, and `spacetimedb-cli build --module-path server`. The module tests currently total 93, including two sender-composition tests.

### Partial or not done

- JMAP requests are still processed one delivery at a time. The dispatcher claims up to 10 jobs for one sending account, but `Email/import` and `EmailSubmission/set` are not batched and partial batch results are not handled.
- Import uncertainty is reconciled by querying the outbox for the correlation header. For submission uncertainty the current code queries `EmailSubmission/query`; it does not use the outbox correlation query as the primary submission reconciliation path, and the planned configurable one-minute wait is not implemented.
- Outbox cleanup occurs after sent state is committed, but cleanup failure is only logged; no durable cleanup retry state exists.
- The current `expire_stale_delivery_claims` reducer was not extended for JMAP rows. Expired JMAP claims become eligible through the dispatcher's claim selection, but restart/catch-up behavior has not been validated against a running database.
- Admin views and generated bindings exist, but the admin UI has no JMAP delivery dashboard, queue counts, per-ingress progress, or retry action yet. Dispatch rate, retry/reconciliation settings, and cleanup behavior are not configurable.
- Live Stalwart behavior (including identity/outbox semantics, header query support, HTTP failure handling, and system/topic credentials), schedule restart recovery, representative throughput, and 1,000-recipient performance have not been tested.
- No production migration or controlled non-production delivery has been run. The old sender daemon, its workspace package, and legacy tables have not been retired.

Native dispatch must remain disabled until a controlled Stalwart test passes, the old sender has been stopped, claimed legacy deliveries have been reconciled, eligible legacy pending work has been migrated, and a rollback point has been agreed.

## Goal and scope

Replace the native `sender` daemon with a SpacetimeDB-owned mail pipeline that:

- Fans inbound mailing-list messages out to one separately addressed message per eligible recipient.
- Sends mailing-list and system messages through Stalwart's JMAP submission API.
- Uses durable SpacetimeDB state for scheduling, claiming, retries, audit, and recovery; no external worker process should be needed.
- Reconciles uncertain submissions before retrying to minimize duplicates. If the result remains unknown, retry is acceptable and may result in a duplicate.
- Retries transient/unknown failures with backoff and exposes known permanent failures rather than retrying them forever.
- Preserves the existing MIME content and attachment behavior: composes per-recipient messages from one canonical source plus per-recipient metadata using `mail-builder`.
- Starts with batches of 10 independent recipient deliveries, grouped by the sending account, then is tuned against Stalwart.
- Targets completing a 1,000-recipient fan-out within 30 minutes under normal conditions; taking longer on very large messages with many attachments is acceptable, and the target is operational guidance, not a hard guarantee.

Breaking API and schema changes are acceptable, but pending work and audit history must not be discarded silently during rollout.

## Evidence and current failure

The supplied EML files have the same `Message-ID`, date, body, and unsubscribe token, but different queue IDs assigned by the receiving Stalwart server. The sender log confirms that the same delivery ID was accepted twice by Stalwart from the same sender instance:

- First successful submission: `20:30:24.071`.
- Second successful submission: `20:30:24.239` (168 ms later).
- Both log entries show the same delivery ID and lease/instance data, but Stalwart returned distinct queued-message IDs.

This confirms repeated processing of the same claimed delivery, rather than a lost SMTP acknowledgment or two sender processes. In `sender/src/main.rs`, each work-loop pass scans the local subscription cache for claimed deliveries. Delivery rows do not have the local in-flight guard used for system mail. A wakeup can therefore cause another pass to see a claimed row before the cache reflects the success reducer's update and send it again.

A short-term mitigation, if the rewrite will take time, is an in-memory in-flight set keyed by delivery ID in the existing daemon. It must be removed when the row is resolved or the send path fails, and must not replace the durable lease/recovery protocol. The new implementation should not make local cache state the authority for whether an email may be submitted.

## Proposed architecture

### Durable work queues

SpacetimeDB remains the source of truth. Reducers perform validation and transactional state transitions; scheduled procedures perform JMAP HTTP operations outside open transactions.

1. Ingress processing resolves active subscriptions and verified recipient addresses, ensures unsubscribe tokens exist, and creates per-recipient delivery records. This runs entirely as a SpacetimeDB procedure, calling `ctx.with_tx()` for each transactional step, so it has no single-transaction size limit.
2. Fan-out runs in one procedure invocation per ingress. The procedure iterates all eligible subscribers and inserts one `MailDeliveryPending` per recipient using multiple small `ctx.with_tx()` calls. `upsert_mail_delivery` is idempotent by delivery key, so re-running the procedure on a partially completed ingress is safe. If the subscriber list grows beyond the range where a single procedure invocation is practical, chunking with a progress cursor can be introduced at that point; it is not planned now.
3. A durable scheduled dispatcher selects a bounded set of due deliveries. It claims the rows transactionally before making network calls, with a unique claim/attempt token and an expiry time long enough to cover the configured HTTP timeout and state updates. The new `claim_next_N_mail_deliveries(n: u32)` reducer atomically claims up to N pending rows in a single transaction, eliminating the N sequential round-trips the previous per-row claim required.
4. The dispatcher groups jobs by topic/system sending account and processes no more than the configured batch size (initially 10). The durable claim, not a local cache or process-local set, prevents concurrent scheduled invocations from owning the same work.
5. A durable recurring schedule or equivalent SpacetimeDB-native scheduled-procedure mechanism wakes the dispatcher. The exact Rust scheduling API and restart/catch-up behavior must be confirmed against the deployed SpacetimeDB server version during the proof of concept. Scheduled execution is best-effort, so all due work remains queryable and recoverable from tables rather than depending on a timer firing exactly on time.

### Delivery state

Use a per-recipient state machine, with names adapted to the final schema:

- `pending`: composed delivery is eligible for import.
- `importing`: a worker has claimed the work and is creating/finding the JMAP Email object.
- `imported`: the JMAP Email ID is durably recorded in SpacetimeDB and is in the dedicated outbox mailbox.
- `submission_unknown` (or an equivalent retryable state): a JMAP submission call may have succeeded but its response was not established.
- `sent`: a successful submission result has been recorded durably in SpacetimeDB.
- `failed`: a known permanent error occurred and needs visibility/action rather than automatic retry.
- A retry timestamp, attempt count, last error, JMAP Email ID, optional EmailSubmission ID, claim owner/token, claim expiry, and timestamps accompany the state as needed.

Every transition must validate the current state and claim token in a SpacetimeDB transaction. Re-running a scheduled invocation for an already completed row must be a no-op. Expired claims are made eligible for recovery by the existing `expire_stale_delivery_claims` scheduler, which must be extended to cover the new JMAP-era job states (`importing`, `submission_unknown`, etc.).

`sent` means Stalwart accepted the JMAP submission. It does not assert that every recipient's remote mail server delivered it to the final mailbox; JMAP delivery status may be recorded separately when Stalwart supplies it.

### JMAP transport and reconciliation

Do not put Missive's `reqwest`/Fetch implementation inside the SpacetimeDB module. Its WASM support targets Fetch-compatible browser/worker hosts; the module should use SpacetimeDB's own `ctx.http` API and direct JMAP JSON requests. Reuse the JMAP protocol patterns already present in `server/src/services/stalwart/client.rs` where appropriate, but use each topic's submission credentials rather than the Stalwart administrator token for ordinary sends.

For each message:

1. Build a deterministic per-recipient RFC 5322 payload using `mail-builder`. Persist or deterministically derive a stable `Message-ID` and a private delivery correlation header (for example, an application-specific delivery ID header) so an import whose response was lost can be found again. The same delivery must produce the same correlation identifiers across retries.
2. Upload the composed raw message through JMAP's upload endpoint. This is one upload per recipient message; it is distinct from batching the subsequent JMAP method calls.
3. Import the raw message into a dedicated outbox mailbox for that topic/system account using `Email/import`. Import batches may contain up to the configured batch size. Persist each successful JMAP Email ID before attempting submission. If the import result is uncertain, search the outbox using the stable correlation header/Message-ID, verify the exact match, and recover the Email ID rather than importing a second copy.
4. Submit imported Email objects using `EmailSubmission/set`, grouped in a JMAP request for up to 10 separate deliveries. Each submission must retain its own Email ID, recipient envelope, identity, and per-item result. Do not combine subscribers into a single multi-recipient message.
5. On a clear successful result, transactionally record the per-delivery submission ID/status and mark the SpacetimeDB job sent. Only after that durable commit may an asynchronous cleanup task delete the imported outbox item. A cleanup failure is safe to retry and must not resubmit a sent message.
6. On an HTTP timeout, connection loss, malformed response, or other uncertain result, do not immediately submit again. Query `EmailSubmission` for the persisted Email ID and inspect the submission result/status. The dedicated outbox mailbox provides a durable recovery path for uncertain imports — unlike `EmailSubmission` records which Stalwart may prune, the outbox Email object and its stable correlation header remain queryable until explicitly deleted by the cleanup task. If no submission can be established after a reconciliation wait of at least 1 minute (configurable), retry with backoff. If the response is partial, process each recipient's result independently.

RFC 8621 provides `EmailSubmission/query` filters by `emailIds`, but permits servers to remove completed submission records. Because the outbox Email object and stable correlation header persist independently of submission records, the primary reconciliation path is always via `Email/query` on the outbox mailbox, not via `EmailSubmission/query`. If Stalwart has discarded a submission record but the outbox Email is still present and undeleted, the submission state remains unknown and must be retried; this is the accepted residual-duplicate risk.

### Batching and throughput

- Initial batch size: 10 separate recipient deliveries per account/JMAP submission request. The dispatcher claims up to N rows atomically using `claim_next_N_mail_deliveries`.
- Keep upload/import/submission stages distinguishable so an uncertain submission does not cause a full batch to be blindly resent.
- The dispatcher may process multiple accounts concurrently within one procedure invocation, bounded by a configurable concurrency limit. Start conservatively (one account at a time), then increase based on Stalwart's configured limits and observed response times.
- The target is roughly 1,000 recipients in 30 minutes under normal conditions. Measure actual throughput — including uploads, import, submission, reconciliation, and cleanup — under a representative load. Very large messages with many attachments may take longer; this is acceptable. The 30-minute figure is operational guidance; scheduled procedures may also be delayed under database load.
- Track batch-level request latency and per-recipient result counts, but persist delivery state per recipient.

### MIME and message composition

The current `sender/src/mail.rs` preserves multipart content and attachments, removes stale unsubscribe tokens from message content, and composes recipient-specific headers. Those observable behaviors are preserved using `mail-builder` inside the SpacetimeDB module.

Use `mail-builder` (a WASM-compatible RFC 5322 / MIME library with no native dependencies) for in-module message composition. The module ingests the canonical inbound RFC 5322 message bytes, parses the structure it needs (body parts, attachments, headers) using a WASM-compatible parser, and composes one outbound RFC 5322 message per recipient with the correct per-recipient headers, unsubscribe token substitution, and From/Reply-To rewrites.

**Storage**: Store one canonical message body and attachment set on the ingress record. Per-recipient delivery records store only the recipient-specific metadata (To, Message-ID, unsubscribe token, List-Unsubscribe URL) needed to deterministically recompose the outbound message. The full per-recipient RFC 5322 payload is recomposed from these inputs at upload time and is not stored in the delivery row. This eliminates the O(N) storage multiplication problem. Confirm actual SpacetimeDB row-size and memory constraints for the canonical payload with representative largest messages before finalizing the storage layout.

Validate HTML, plain text, multipart/alternative, inline media, attachments, encodings, custom headers, and unsubscribe-token cleanup against fixture tests before retiring the existing composer.

### System mail and credentials

Use the same queue, claims, retry, JMAP import/submission, reconciliation, and audit pipeline for system mail, with a distinct system sending account. Preserve the current distinction between topic app-password credentials and system-account credentials.

- Read topic credentials from the existing private SpacetimeDB credential table (`TopicAppPassword`).
- Store the system account username and password as SpacetimeDB environment secrets declared via `#[spacetimedb::env]` on a module-private `Env` struct and read via `ctx.env.SYSTEM_JMAP_USERNAME()` / `ctx.env.SYSTEM_JMAP_PASSWORD()`. These replace the current `SMTP_SYSTEM_USERNAME` / `SMTP_SYSTEM_PASSWORD` environment variables used by the legacy sender daemon. Supply values as shell environment variables (or in the `env` map in `spacetime.json`) and run `spacetime publish --env-only` without redeploying the module binary. The CLI's `--env` option selects a configuration profile; it does not set a `KEY=VALUE` pair.
- Validate that each credential can discover the correct JMAP account, use an authorized sending identity, import into the dedicated outbox, and submit from the expected From address.
- Never include credentials in logs, job rows, client-visible views, or procedure return values. The SpacetimeDB env API does not expose secret values to clients or through table subscriptions.

### Timeout and HTTP behavior

The official procedure documentation states that outbound HTTP defaults to 30 seconds and a requested timeout is capped at 180 seconds. This is the SpacetimeDB-to-Stalwart HTTP timeout, not Stalwart's own server timeout. The existing project helper requests 30 seconds. The API docs previously inspected conflict with the current official documentation, so verify the actual deployed SpacetimeDB server version and run a controlled timeout test before setting production values. Do not hold a database transaction open during any JMAP HTTP request.

## Implementation phases

### Phase 0: Prevent current duplicate sends (optional interim step)

- Add a delivery in-flight guard to the current sender, keyed by delivery ID, to stop repeated processing while a success update is still pending in the local subscription cache.
- Add a regression test for duplicate wakeups/cache updates causing one claimed row to be submitted only once within one worker process.
- Deploy this only if the full replacement will not be ready promptly; it is a mitigation, not the final delivery protocol.

### Phase 1: Verify SpacetimeDB and Stalwart capabilities

- Record the actual SpacetimeDB server/CLI version and validate scheduled Rust procedures, recurring scheduling, restart recovery, and procedure execution limits in a test database.
- Exercise `ctx.http` with the deployment's actual timeout and inspect behavior on slow responses, dropped connections, non-2xx HTTP status, and malformed JMAP JSON.
- Verify that `#[spacetimedb::env]` secrets declared on a module-private struct are readable via `ctx.env` in reducers, procedures, and HTTP handlers, and confirm they are never exposed to clients. Test `spacetime publish --env-only` for secret rotation without module redeployment.
- Verify JMAP session discovery, account/identity resolution, raw-message upload, `Email/import`, `EmailSubmission/set`, and `EmailSubmission/query` with topic and system credentials.
- Verify Stalwart's outbox mailbox behavior, the query support for the correlation header, and that deleted outbox Emails are truly gone (confirming the cleanup path is safe). Verify that `Email/query` on the outbox by correlation header works as the primary reconciliation path.
- Confirm per-account batch size/request-size limits and partial-result behavior. Establish initial dispatch concurrency/rate settings.
- Prototype `mail-builder` message composition in the WASM target with a representative inbound RFC 5322 message, verifying multipart/alternative, inline images, attachments, and custom headers round-trip correctly.

### Phase 2: Define schema and transition helpers

- Add `claim_next_N_mail_deliveries(n: u32)` reducer that atomically claims up to N pending delivery rows in one transaction and returns the claimed delivery IDs.
- Add durable per-recipient job state and indexes for status/next-attempt time, account/topic, and ingress ID.
- Add a durable scheduled dispatcher record.
- Add or adapt delivery audit events to record import, submission, reconciliation, retry, permanent failure, sent, and cleanup outcomes.
- Implement transactional claim, complete, retry, permanent-failure, and cleanup-state reducers/helpers.
- Extend `expire_stale_delivery_claims` to cover the new JMAP-era job states (`importing`, `submission_unknown`, etc.). This is an extension of the existing scheduled reducer, not new work.
- Ensure claim tokens fence stale procedure completions so an old invocation cannot overwrite a newer claim's result.
- Define the migration plan for existing pending, claimed, temporary-failed, done, system-mail, and audit rows. Back up/export before any schema reset or destructive publication.
- Declare `#[spacetimedb::env]` secrets struct for system JMAP credentials.

### Phase 3: Move fan-out and composition into the module

- Move recipient selection, address verification, deduplication, unsubscribe-token resolution, subject/header composition, and MIME handling into the server module using `mail-builder`.
- Preserve the current eligibility rules: active subscriptions, verified account email addresses, recipient deduplication, per-recipient To, topic From/Reply-To, deterministic Message-ID, list headers, unsubscribe behavior, and attachment handling.
- Implement fan-out as a procedure that processes all eligible subscribers in one invocation using `ctx.with_tx()` per insert. `upsert_mail_delivery` idempotency ensures safety on re-runs. Store per-recipient metadata only (not full raw message) in delivery records.
- Keep the canonical inbound message bytes available on the ingress record until all recipient delivery records are created and all uploads have completed or failed terminally.

### Phase 4: Implement JMAP import, submission, and recovery procedures

- Implement session/config resolution with no long-lived process-local cache assumptions; cache only data whose lifetime and invalidation are explicit, or rediscover when needed.
- Implement raw-message upload (composing per-recipient RFC 5322 from canonical source + metadata via `mail-builder`) and import to the dedicated outbox, then persist Email IDs before submission.
- Implement per-account submission batching (initially 10) using `claim_next_N_mail_deliveries` and parse every JMAP method result, including partial successes and `notCreated` errors.
- Implement reconciliation by Email ID and correlation header before retrying an uncertain operation. Reconciliation queries the outbox mailbox for the correlation header and waits at least 1 minute (configurable via `JMAP_SUBMISSION_UNKNOWN_MIN_WAIT`) before treating the status as unknown-and-retryable.
- Keep outbox cleanup strictly after the SpacetimeDB sent state commits.
- Handle HTTP/JMAP auth/config and invalid-address/content failures as permanent; treat timeouts, connection loss, and retryable server errors as uncertain/transient with capped exponential backoff and jitter.
- Add limits to attempts/parallel dispatch only if an operationally safe dead-letter/manual-review policy is agreed; do not silently discard transient jobs.

### Phase 5: Schedule autonomous dispatch and system mail

- Implement a durable recurring scheduled dispatcher that claims due work using `claim_next_N_mail_deliveries` and invokes the JMAP pipeline without an external sender process.
- Ensure only a bounded number of jobs/accounts are in flight; on module restart, expired claims and due retries must become eligible again.
- Route system mail through the same pipeline with system-account credentials (read from `ctx.env`) and the same duplicate-minimizing reconciliation rules.
- Verify that schedule delays do not lose jobs and that backlog recovery catches up without creating an uncontrolled burst.

### Phase 6: UI, observability, and operational controls

- Update admin/status views and generated bindings for the new state model; regenerate bindings rather than editing generated files manually.
- Show counts for queued, importing, submitting/uncertain, sent, retrying, and permanent failures, plus per-ingress progress.
- Record per-job delivery ID, account/topic, attempt number, batch correlation ID, JMAP Email/Submission IDs, status, timestamps, and sanitized error detail. Never log passwords, Authorization headers, message bodies, or full recipient lists unnecessarily.
- Expose/configure batch size, dispatch concurrency/rate, retry backoff bounds, reconciliation wait, and outbox cleanup behavior through server-side SpacetimeDB environment values or admin-settable config rows with safe defaults.

### Phase 7: Test, migrate, and retire the daemon

- Run the new pipeline in a non-production database/account with a controlled mailbox before enabling production dispatch.
- Pause or disable the old sender before enabling the scheduled dispatcher; never let both paths submit the same queue concurrently.
- Drain or migrate current pending work. For claimed/in-flight jobs, use logs and Stalwart submission state to decide whether to mark sent, reconcile, or retry; document that unresolved jobs may duplicate if retried.
- Back up data before any breaking schema publication. Validate post-publish state and scheduled-job recovery before removing old tables/code.
- Retire the `/sender` workspace package, tasks, secrets, and deployment. The `sender` crate carries no reusable code into the new architecture — its `module_bindings/` are generated artifacts and `mail.rs` / `main.rs` are replaced by in-module equivalents — and is removed in full after production verification and an agreed rollback point.

## Validation criteria

### Correctness and recovery

- Repeated scheduler invocations, duplicate wakeups, overlapping claims, and stale claim completions do not submit the same known delivery twice.
- A clear JMAP per-item success becomes `sent` exactly once in SpacetimeDB; a repeated completion call is idempotent.
- Partial batch success updates only the successful deliveries and retries/fails only the corresponding remaining items.
- Simulated failures at each boundary (upload, import before/after response, submission before/after response, SpacetimeDB commit, cleanup) recover from durable state. Uncertain submission outcomes are reconciled before resubmission.
- Expired worker claims are recoverable after a procedure/module restart; pending and retry state is not lost.
- Permanent JMAP errors stop retrying and are visible to admins; transient and unknown errors back off and remain recoverable.

### Message fidelity and security

- One independent message is created for each recipient; recipients are not exposed to one another.
- MIME fixtures verify plaintext, HTML, multipart alternatives, attachments, inline resources, encodings, and preservation/sanitization of headers and unsubscribe tokens.
- Topic and system messages use the proper From identity and credentials. Invalid credentials do not trigger endless message-by-message retry storms.
- Secrets are read exclusively from `ctx.env` and are absent from logs/status tables visible to clients.

### Performance and operations

- A representative 1,000-recipient test completes within 30 minutes at the initial batch size/rate configuration under normal message sizes, with measurements for upload, import, submission, reconciliation, and scheduler delay.
- Tests include Stalwart rate limiting, a slow JMAP response near the configured timeout, and an unavailable Stalwart instance followed by recovery.
- Monitoring can distinguish queue delay, JMAP latency, retry backlog, permanent failures, and uncertain submissions.

## Known limitations and risks

- SpacetimeDB scheduled procedures are best-effort and may be delayed under load; the delivery deadline is therefore verified operationally, not guaranteed by the scheduler alone.
- No transaction can atomically commit both SpacetimeDB state and Stalwart's external submission. If a submission response is lost and Stalwart no longer exposes a matching submission record, a retry may still duplicate a message. The agreed policy accepts this residual risk.
- The outbox Email object and correlation header are the primary reconciliation path. If Stalwart loses or corrupts the outbox itself, the reconciliation fallback is unavailable. This risk is accepted.
- Raw per-recipient uploads increase HTTP traffic. Per-recipient recomposition via `mail-builder` adds CPU cost but eliminates per-recipient storage in the database. Measure the actual pipeline at 1,000 recipients before increasing concurrency or batch size.
- Very large messages (many large attachments) will take longer to process per batch; this is accepted and does not constitute a failure of the pipeline.
