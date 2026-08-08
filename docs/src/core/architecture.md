# Architecture

The Kommunikationszentrum consists of three main running components that work together to provide email management for the SoLaWi project.

## System Overview

```d2
{{#include architecture-overview.d2}}
```

## Data Flow

```d2
{{#include architecture-data-flow.d2}}
```

**Legend:**
- **1: Email Processing** – Incoming emails are processed by Stalwart MTA and delivered to SpacetimeDB via the module's HTTP routes where the delivery is validated and persisted.
- **2: User Management** – User changes are managed in Django and synchronized to SpacetimeDB over the module's user-sync HTTP route.
- **3: Subscription Management** – Users manage their subscriptions in the Dioxus WebAssembly frontend, which communicates with SpacetimeDB.
- **4: Admin Interface** – Admin actions are performed in the Admin UI and reflected in SpacetimeDB.

## Components

### SpacetimeDB Server (crate: `server`)
- Database and business logic layer
- WASM modules with Rust reducers and HTTP handlers
- Exposes module-specific HTTP routes under `/v1/database/:name/route/{*path}`
- Real-time subscriptions for UI updates

### Admin Web Interface and user subscription management (crate: `admin`)
- Dioxus WebAssembly frontend
- OAuth authentication via Django
- Subscription management interface

### Sender Service (crate: `sender`)
- Processes incoming mails received via SpacetimeDB
- Validates email format, sender, and subscription status
- Publishes messages to subscribed users via email

### External Dependencies
- **Django solawispielplatz**: User management and OAuth provider
- **Stalwart MTA**: Email server that posts stage hooks directly to the module HTTP routes

## Authentication Flow

```d2
{{#include architecture-auth-flow.d2}}
```

The system uses OAuth 2.0 with Django as the identity provider. JWT tokens are validated by SpacetimeDB for all authenticated WebSocket connections. External systems (MTA, Django sync) use module HTTP routes secured with bearer tokens and permissions.
