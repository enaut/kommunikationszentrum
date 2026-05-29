# Development Environment
**Target System**: Fedora 44 with the following tools available among others:
- Package manager: `dnf`
- Init system: `systemd`  
- Display server: `Wayland` with GNOME desktop
- Container runtime: `podman`
- Rust toolchain: `cargo`
- Dioxus CLI: `dx`
- Documentation: `mdbook`
- Database: `spacetime` CLI

# Documentation Guidelines
**Location**: All project documentation must be written in the mdbook located at `docs/`

**Language**: All documentation and code comments must be in English

**Quality Standards**:
- Write comprehensive documentation that enables developers to understand both the code structure and usage patterns

**Documentation Practices**:
- Always ask whether to update documentation after making significant changes:
  - update the mdbook documentation
  - update these copilot instructions when making architectural changes

**Content Restrictions**:
- Do NOT invent design principles or project goals unless explicitly requested
- Do NOT add "planned", "future", or "roadmap" sections
- Do NOT include "recommended practices" or "best practices" sections
- Avoid overly verbose text and marketing buzzwords
- Write informative, concise documentation

**Technical Requirements**:
- Use `mdbook-graphviz` for diagrams (render dot graphs where appropriate)
- Structure content using nested chapters instead of second-level headlines (## doesn't work properly)

**Project Context**: 
This is a Community Supported Agriculture (SoLaWi) email management system that processes and routes emails based on user subscriptions to mailing list categories.

# Project Structure
This is a Rust workspace with multiple components organized as follows:

```
kommunikationszentrum/
├── admin/                    # Dioxus web frontend (port 8080)
│   ├── assets/              # Bootstrap CSS and static web assets
│   └── src/                 # Rust source for the web interface
├── Cargo.toml               # Root workspace configuration
├── docs/                    # Project documentation (mdbook)
│   ├── book.toml           # Mdbook configuration
│   └── src/                # Markdown documentation sources
├── server/                  # SpacetimeDB module (port 3000)
│   └── src/                # Database schema and reducer logic
├── webhook-proxy/           # HTTP API gateway (port 3002)
│   └── src/                # Axum web server handling MTA hooks and user sync
├── dioxusllms.txt          # Dioxus framework documentation reference  
└── spacetimellms.txt       # SpacetimeDB documentation reference
```

**Key Files**:
- `Cargo.toml`: Defines the Rust workspace and component dependencies
- `docs/dioxusllms.txt`: Reference documentation for the Dioxus web framework
- `docs/whichspacetimellms.txt`: Reference documentation for SpacetimeDB database system

# Running the System Components

## solawispielplatz (Django Backend)
**Purpose**: User management and OAuth provider for the SoLaWi project  
**Port**: 8000 (default)  
**Python Environment**: `/home/dietrich/.envs/Solawis/current/bin/python`

**Commands**:
```bash
# Start Django development server
/home/dietrich/.envs/Solawis/current/bin/python /home/dietrich/Projekte/Source/solawispielplatz/src/manage.py runserver

# Synchronize users to SpacetimeDB
/home/dietrich/.envs/Solawis/current/bin/python /home/dietrich/Projekte/Source/solawispielplatz/src/manage.py sync_users_to_spacetimedb
```

## webhook-proxy (HTTP API Gateway)
**Purpose**: Handles MTA hooks from Stalwart email server and user synchronization  
**Port**: 3002 (default)

**Commands**:
```bash
# Start the webhook proxy server
cargo run --package webhook-proxy
```

## server (SpacetimeDB Module)
**Purpose**: Database and business logic layer  
**Port**: 3000 (default)

**Commands**:
```bash
# Start SpacetimeDB server
spacetime start

# Build and publish schema/reducers to 'kommunikation' database
spacetime publish --project-path server kommunikation

# Reset database when schema changes (use with caution)
spacetime publish --project-path server kommunikation -c
```

## admin (Dioxus Web Frontend)
**Purpose**: Web interface for subscription management and administration  
**Port**: 8080 (default)

**Commands**:
```bash
# Start development server with WebAssembly support
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' dx serve --package admin --platform web
```

Allways use `dx` or the vscode task "Start Admin Web UI", Do not use `cargo` commands for building or launching dioxus admin.

# Architecture Overview

The Kommunikationszentrum is a distributed email management system for the SoLaWi project that consists of four main components:

## Component Overview

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

## Data Flow

1. **Email Processing**: Stalwart MTA → webhook-proxy → SpacetimeDB
2. **User Management**: Django → webhook-proxy → SpacetimeDB
3. **Admin Interface**: Admin UI ↔ SpacetimeDB (OAuth via Django)

## Components Description

### 1. **SpacetimeDB Server** (`/server`)
- **Purpose**: Core database and business logic layer
- **Technology**: SpacetimeDB WASM modules
- **Responsibilities**:
  - Email category management (`message_categories`)
  - Subscription management (`subscriptions`)
  - MTA hook processing and logging
  - User data storage
  - IP blocking lists (`blocked_ips`)

### 2. **Webhook Proxy** (`/webhook-proxy`)
- **Purpose**: HTTP API gateway between external systems and SpacetimeDB
- **Technology**: Rust + Axum web framework
- **Endpoints**:
  - `/mta-hook`: Receives MTA hooks from Stalwart email server
  - `/user-sync`: Synchronizes users from Django solawispielplatz
- **Responsibilities**:
  - MTA hook validation and processing
  - User synchronization from Django
  - HTTP-to-SpacetimeDB protocol translation

### 3. **Admin Web Interface** (`/admin`)
- **Purpose**: User-facing web application for subscription management
- **Technology**: Rust + Dioxus (WebAssembly frontend)
- **Features**:
  - OAuth authentication via Django
  - Personal subscription management
  - Admin: User and category management
  - Real-time updates via SpacetimeDB subscriptions

### 4. **External Dependencies**
- **solawispielplatz Django**: User management and OAuth provider
- **Stalwart MTA**: Email server that sends hooks to webhook-proxy
- **OAuth Flow**: Django → Admin UI authentication

## Email Processing Flow

```
Incoming Email → Stalwart MTA → MTA Hook → webhook-proxy
                                              ↓
SpacetimeDB ← Process & Log ← Validate Categories & Subscriptions
     ↓
Decision: ACCEPT / REJECT / QUARANTINE
```

## Key Tables in SpacetimeDB

- **`person`**: User accounts
- **`message_categories`**: Email categories (mailing lists)
- **`subscriptions`**: User subscriptions to categories
- **`mta_connection_log`**: Connection-level MTA logs
- **`mta_message_log`**: Message-level MTA logs  
- **`blocked_ips`**: IP blacklist for spam protection

## Development Workflow

1. **Schema Changes**: Modify `/server` → `spacetime publish`
2. **API Changes**: Modify `/webhook-proxy` → `cargo run`
3. **UI Changes**: Modify `/admin` → `dx serve`
4. **User Sync**: Run `manage.py sync_users_to_spacetimedb` in Django
