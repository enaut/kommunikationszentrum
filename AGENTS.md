## Agent skills

### Issue tracker

GitHub Issues via `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical roles: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout (`GLOSSARY.md` + `docs/adr/`). See `docs/agents/domain.md`.

## Architecture & Delivery Pipeline

### Outbound Mail Delivery (JMAP Native)

The outbound email delivery pipeline is migrating from the external `sender` daemon (`/sender`) to an in-module SpacetimeDB-native scheduled pipeline in `/server`.
- **Sender Daemon Deprecation**: The `/sender` crate is deprecated and will be removed completely. Do not add new features, refactors, or dependencies to `sender/`.
- **Module Bindings**: Client bindings (`spacetimedb-cli generate`) are generated only for `admin/src/module_bindings/` (using `--lang dioxus`). The `sender` daemon is being retired completely and therefore needs no `module_bindings/`. Do not generate or maintain `sender/src/module_bindings/`.
- **MIME & Composition**: In-module message composition uses `mail-builder` (WASM-compatible). Raw messages are not stored per recipient row (Option A); canonical content is kept on the ingress record and per-recipient RFC 5322 messages are assembled at upload time.
- **Secrets**: Use SpacetimeDB typed environment secrets declared via `#[spacetimedb::env]` and read via `ctx.env` (SpacetimeDB v2.12+). Do not store secrets in `.env` files, plaintext database tables, or client-visible views.
