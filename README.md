# Kommunikationszentrum - SoLaWi Email Management System

A Community Supported Agriculture (SoLaWi) email management system that processes and routes emails based on user subscriptions to mailing list categories.

## Quick Start

### Automated Development Setup

The easiest way to start all development services is to use the vscode tasks.

This will automatically start all required services:
- SpacetimeDB Server (port 3000)
- Django Backend (port 8000)  
- Webhook Proxy (port 3002)
- Admin Web UI (port 8080)

### Service Management

```bash
# Check service status
make status

# View recent logs
make logs

# Follow logs in real-time  
make logs-follow

# Stop all services
make stop

# Restart everything
make restart

# Clean up logs and stop services
make clean
```

### VS Code Integration

If you're using VS Code:
1. Open the workspace: `kommunikationszentrum.code-workspace`
2. Use **Ctrl+Shift+P** → "Tasks: Run Task" → "Start All Development Services"
3. Or use **F5** to start with debugging support

## Architecture Overview

The system consists of four main components:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Admin Web UI  │    │ Webhook Proxy   │    │   SpacetimeDB   │
│   (Dioxus)      │◄──►│   (Axum HTTP)   │◄──►│   (Database)    │
│   Port 8080     │    │   Port 3002     │    │   Port 3000     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       ▲
         │                       │
         ▼                       ▼
┌─────────────────┐    ┌─────────────────┐
│ OAuth Provider  │    │ Stalwart MTA    │
│ solawispielplatz│    │   (External)    │
│ Django Port 8000│    └─────────────────┘
└─────────────────┘
```

### Components

1. **SpacetimeDB Server** (`/server`): Core database and business logic layer
2. **Webhook Proxy** (`/webhook-proxy`): HTTP API gateway for MTA hooks and user sync
3. **Admin Web Interface** (`/admin`): Dioxus WebAssembly frontend
4. **Django Backend** (external): User management and OAuth provider

## Manual Setup

### Prerequisites

- [Rust](https://rustup.rs/) with `wasm32-unknown-unknown` target
- [SpacetimeDB CLI](https://spacetimedb.com/install)
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started): `cargo install dioxus-cli`
- Python 3.x with Django environment at `/home/dietrich/.envs/Solawis/current/bin/python`

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

#### 3. Start Django Backend

```bash
/home/dietrich/.envs/Solawis/current/bin/python /home/dietrich/Projekte/Source/solawispielplatz/src/manage.py runserver
```

#### 4. Start Webhook Proxy

```bash
cargo run --package webhook-proxy
```

#### 5. Start Admin Web UI

```bash
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' dx serve --package admin --platform web
```

#### 6. Sync Users to SpacetimeDB

```bash
cd /home/dietrich/Projekte/Source/solawispielplatz
/home/dietrich/.envs/Solawis/current/bin/python src/manage.py sync_users_to_spacetimedb
```

## Development Commands

```bash
# Build all Rust components
make build

# Run tests
make test

# Sync users manually
make sync-users

# Reset database (DESTRUCTIVE!)
make reset-db

# Start documentation server
make dev-docs
```

## Service URLs

- **Admin Web UI**: http://localhost:8080
- **Django Backend**: http://localhost:8000
- **Webhook Proxy**: http://localhost:3002
- **SpacetimeDB**: http://localhost:3000

## Documentation

Complete documentation is available in the `docs/` directory:

```bash
# Start documentation server
cd docs && mdbook serve --open
```

## Troubleshooting

### Port Conflicts
If you encounter port conflicts, check which services are running:
```bash
make status
```

### Service Logs
View detailed logs for debugging:
```bash
make logs          # Recent logs
make logs-follow   # Real-time logs
```

### Clean Restart
For a complete clean restart:
```bash
make clean
make start
```
