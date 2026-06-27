# Architecture

The Kommunikationszentrum consists of three main running components that work together to provide email management for the SoLaWi project.

## System Overview

```d2
direction: down
admin_ui: "Admin Web UI\n(Dioxus)\nPort 8080" {
  style.fill: lightblue
}
spacetimedb: "SpacetimeDB\n(Database + HTTP Routes)\nPort 3000" {
  style.fill: lightcoral
}
django: "OAuth Provider\nsolawispielplatz\nDjango Port 8000" {
  style.fill: lightyellow
}
django_user: "User database\nsolawispielplatz\nDjango Port 8000" {
  style.fill: lightyellow
}
stalwart: "Stalwart MTA\n(External)" {
  style.fill: lightgray
}

admin_ui -> spacetimedb: "WebSocket\nSubscriptions" {
  style.stroke: blue
}
admin_ui -> django: "OAuth\nLogin" {
  style.stroke: orange
}
django_user -> admin_ui: "User Synchronization" {
  style.stroke: orange
}
stalwart -> spacetimedb: "MTA\nHooks (HTTP)" {
  style.stroke: red
}
django -> spacetimedb: "User Sync (HTTP)" {
  style.stroke: green
}

core: "Core Kommunikationszentrum" {
  admin_ui
  spacetimedb
}
external: "External Systems" {
  django
  django_user
  stalwart
}
```

## Data Flow

```d2
direction: right
incoming_email: "Incoming\nEmail" { style.fill: lightcoral }
user_changes: "User\nChanges" { style.fill: lightblue }
admin_actions: "Admin\nActions" { style.fill: lightgreen }

stalwart_mta: "Stalwart\nMTA" { style.fill: lightgray }
spacetimedb: "SpacetimeDB" { style.fill: lightcoral }
admin_ui: "Admin UI" { style.fill: lightblue }
django: "Django" { style.fill: lightyellow }

incoming_email -> stalwart_mta -> spacetimedb: "1" { style.stroke: red }
user_changes -> django -> spacetimedb: "2" { style.stroke: blue }
admin_actions -> admin_ui -> spacetimedb: "3" { style.stroke: green }
admin_ui -> django: "OAuth" { style.stroke: orange; style.stroke-dash: 5 }
spacetimedb -> admin_ui: "WebSocket\nUpdates" { style.stroke: purple; style.stroke-dash: 2 }
```

**Legend:**
- **1: Email Processing** – Incoming emails are processed by Stalwart MTA and delivered to SpacetimeDB via the module's HTTP routes where the delivery is validated and persisted.
- **2: User Management** – User changes are managed in Django and synchronized to SpacetimeDB over the module's user-sync HTTP route.
- **3: Admin Interface** – Admin actions are performed in the Admin UI and reflected in SpacetimeDB.

## Components

### SpacetimeDB Server (`/server`)
- Database and business logic layer
- WASM modules with Rust reducers and HTTP handlers
- Exposes module-specific HTTP routes under `/v1/database/:name/route/{*path}`
- Real-time subscriptions for UI updates

### Admin Web Interface (`/admin`)
- Dioxus WebAssembly frontend
- OAuth authentication via Django
- Subscription management interface

### External Dependencies
- **Django solawispielplatz**: User management and OAuth provider
- **Stalwart MTA**: Email server that posts stage hooks directly to the module HTTP routes

## Authentication Flow

```d2
direction: right
user: "User" { style.fill: lightblue }
django: "OAuth Login\n(Django)" { style.fill: lightyellow }
jwt: "JWT Token" { style.fill: lightgreen }
spacetimedb: "SpacetimeDB\nConnection" { style.fill: lightcoral }

user -> django: "OAuth Login" { style.stroke: orange }
django -> jwt: "Issue JWT" { style.stroke: green }
jwt -> spacetimedb: "Authenticate\nConnection" { style.stroke: blue }
```

The system uses OAuth 2.0 with Django as the identity provider. JWT tokens are validated by SpacetimeDB for all authenticated WebSocket connections. External systems (MTA, Django sync) use module HTTP routes secured with bearer tokens and permissions.
