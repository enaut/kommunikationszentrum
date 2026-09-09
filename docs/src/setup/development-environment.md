# Development Environment

Guide to configuring and running the local development environment for Kommunikationszentrum.

---

## Architecture Components

A complete local development environment consists of the following components:

1. **SpacetimeDB Server** (`:3000`): Database and execution engine for reducers, procedures, and embedded HTTP routes (`/route/mta-hook`, `/route/user-sync`, `/route/mailing-list/unsubscribe`).
2. **Admin Web App** (`:8080`): Dioxus WebAssembly frontend for managing categories, subscriptions, identities, and webhook tokens.
3. **Sender Daemon**: Background service that claims ingress and delivery jobs from SpacetimeDB and delivers emails via SMTP.
4. **Django Backend (solawispielplatz)** (`:8000`): OAuth 2.0 / OIDC provider and source of user accounts.
5. **Stalwart Mailserver** (`:8093` / `:25`): Local MTA for inbound mail hooks and JMAP mailbox management.
6. **Observability Stack (Grafana / Loki / Tempo / Alloy)**: Optional local telemetry collector running via `podman-compose` in `grafana/`.

---

## Environment Setup

1. **Global / Admin Configuration**:
   Copy the master development template to `.env`:
   ```bash
   cp .env/.env.example .env
   ```

2. **Sender Daemon Configuration**:
   The sender daemon requires its own environment configuration. Copy the sender template:
   ```bash
   cp .env/.env.sender.example .env/.env.sender
   # Alternatively, compatibility file: .env/.env.webhook-proxy
   ```

3. **Stalwart Mailserver Configuration**:
   Create Stalwart API credentials and store them in `.env/.env.stalwart-jmap`:
   ```bash
   cp .env/.env.stalwart-jmap.example .env/.env.stalwart-jmap
   ```

---

## Running Development Services

### Using Zed Tasks

The project includes pre-configured tasks in `.zed/tasks.json` (`Ctrl+Shift+P` → `task: spawn`):

- **Start SpacetimeDB**: `spacetime start`
- **Publish spacetime module**: `sh -lc '. .env/.env.stalwart-jmap && spacetime publish -p server kommunikation'`
- **Start Dioxus (Admin)**: `dx serve --package admin --platform web`
- **Start Sender**: `sh -lc '. .env/.env.webhook-proxy && cargo run --package sender'`
- **Start Grafana**: `podman-compose up` (inside `grafana/`)
- **Serve Docs**: `mdbook serve docs --port 3022`

### Using the Command Line

```bash
# 1. Start SpacetimeDB
spacetime start

# 2. Publish the module
spacetime publish --project-path server kommunikation

# 3. Launch Admin Web UI
dx serve --package admin --platform web

# 4. Run the Sender Daemon
sh -lc '. .env/.env.sender.example && cargo run --package sender'
```

---

## Next Steps

- Consult the [Configuration Reference](configuration.md) for detailed descriptions of all options.
- Review [Environment Variables Reference](../reference/environment-variables.md) for the complete variable catalog.
