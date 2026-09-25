# Kommunikationszentrum - SoLaWi Email Management System

A Community Supported Agriculture (SoLaWi) email management system that processes and routes emails based on user subscriptions to message topics (mailing lists) classified by category tags.

The main documentation is available at: [![Documentation](https://github.com/enaut/kommunikationszentrum/actions/workflows/deploy-docs.yaml/badge.svg)](https://enaut.github.io/kommunikationszentrum/)

## Quick Start

### Service Management
It could make sense to create local zed tasks to start spacetimedb and solawis in the user settings (ctrl+shift+p→open tasks) see also: https://enaut.github.io/kommunikationszentrum/setup/installation.html.

#### Externals
1. start Django-Solawis
1. start SpacetimeDB

#### Zed tasks
Some zed-editor tasks are defined in the project settings:

1. "Publish spacetime module"
1. "Start Sender" (Starting the email sender that interacts with stalwart and spacetimedb)
1. "Start Grafana" (Starting Grafana for log reading)
1. "Start Dioxus" (Starting the admin interface)
1. "Serve Docs" (Optional to read the docs)

### Environment Variables

In the `.env` directory are some example files for the different purposes. Rename `*.example` to `*` and adjust the values as needed.

```bash
cp .env.example .env
```

## Current Architecture

The system is organized around four cooperating layers:

- An **OIDC Provider** (Nextcloud, Keycloak, Authentik, or Django) provides user identity and authentication via OAuth 2.0 / OpenID Connect.
- **Stalwart** owns the mail transfer layer, domain inventory, and mailbox provisioning.
- **SpacetimeDB** stores the canonical routing state, accounts, subscriptions, and delivery pipeline.
- The **sender daemon** claims outbound work, sends mail, and retries transient SMTP failures.

```mermaid
graph TD
    Admin["Admin Web UI<br/>Dioxus<br/>Port 8080"]
    DB["SpacetimeDB<br/>Database + reducers<br/>Port 3000"]
    OIDC["OIDC Provider<br/>Nextcloud / Keycloak / Django<br/>Port 8000 / 443"]
    Stalwart["Stalwart MTA<br/>SMTP + JMAP + domains"]
    Sender["Sender Daemon<br/>delivery + retry queue"]
    Queue["Temporary failure queue"]
    Relay["Outbound SMTP relay"]

    Admin <--> DB
    Admin --> OIDC
    OIDC -. Optional sync .-> DB
    Stalwart --> DB
    Stalwart <--> DB
    DB --> Sender
    Sender --> Queue
    Queue --> Sender
    Sender --> Stalwart
    Sender --> Relay
```

### Components

1. **SpacetimeDB Server** (`/server`): canonical state for accounts, topics, categories, subscriptions, domains, and delivery jobs
2. **Admin Web Interface** (`/admin`): Dioxus frontend for member and admin workflows
3. **Sender Daemon** (`/sender`): SMTP delivery worker with retry, lease expiry, and queue recovery
4. **External integrations**: Django OAuth/user sync and Stalwart MTA/domain management

## Manual Setup

### Prerequisites

- [Rust](https://rustup.rs/) with `wasm32-unknown-unknown` target
- [SpacetimeDB CLI](https://spacetimedb.com/install)
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started): `cargo install dioxus-cli`

### Manual Service Startup

If you prefer to start services individually:

#### 1. Start SpacetimeDB Server

```bash
spacetime start
```

#### 2. Publish Database Schema

```bash
spacetime publish --project-path server kommunikation
```

#### 3. Start OIDC Provider (e.g. Django)
If using local Django for development:
```bash
/home/dietrich/.envs/Solawis/current/bin/python /home/dietrich/Projekte/Source/solawispielplatz/src/manage.py runserver
```
*(Or use an external OIDC provider like Nextcloud or Keycloak by configuring `OIDC_ISSUER_URL`.)*

#### 4. Start Admin Web UI

```bash
dx serve --package admin --platform web
```

#### 5. (Optional) Sync Users from External System

If synchronizing accounts from Django or an external database:
```bash
cd /home/dietrich/Projekte/Source/solawispielplatz
/home/dietrich/.envs/Solawis/current/bin/python src/manage.py sync_users_to_spacetimedb
```
*(Note: OIDC login will also auto-provision accounts on first connection via `register_self`.)*

## Documentation

Complete documentation is available in the `docs/` directory:

```bash
# Start documentation server
cd docs && mdbook serve --open
```
