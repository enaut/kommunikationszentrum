## Agent skills

### Issue tracker

GitHub Issues via `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical roles: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout (`GLOSSARY.md` + `docs/adr/`). See `docs/agents/domain.md`.

## Architecture & Delivery Pipeline

### Outbound Mail Delivery (JMAP Native)

Outbound mail is dispatched by the scheduled SpacetimeDB module in `/server`; no external sender worker is part of the workspace. Keep delivery state durable in module tables and perform JMAP network requests through the procedure HTTP API.
- **Migration Data**: Legacy SMTP queue tables remain available to the migration procedure and for reconciliation. Preserve pending or claimed records until they are explicitly reconciled or migrated; do not reset production data as part of routine schema work.
- **Module Bindings**: Regenerate Dioxus client bindings after schema or reducer changes with `spacetimedb-cli generate --lang=dioxus -p server -o admin/src/module_bindings/`. Preserve intentional hand-maintained extensions in generated bindings, such as the JMAP actor formatter.
- **MIME & Composition**: In-module message composition uses WASM-compatible `mail-builder`. Store canonical content on the ingress record and compose recipient-specific RFC 5322 messages at upload time rather than duplicating raw messages in delivery rows.
- **Secrets**: Use SpacetimeDB typed environment secrets declared via `#[spacetimedb::env]` and read via `ctx.env`. Do not store secrets in plaintext database tables or client-visible views.
