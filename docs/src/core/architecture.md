# Architecture

The Kommunikationszentrum is built from project-owned runtime services and external infrastructure. The canonical state lives in SpacetimeDB, while Django and Stalwart provide the identities and mail transport layer that the module integrates with.

## System Overview

```d2
{{#include architecture-overview.d2}}
```

## Data Flow

```d2
{{#include architecture-data-flow.d2}}
```

**Current flow:**
- **1: Inbound mail** – Stalwart posts MTA hook data to SpacetimeDB, which normalizes the message and validates category, account, and recipient state.
- **2: Identity sync** – Django pushes user, subscription, and category data into the module so the canonical membership model stays current.
- **3: Domain and mailbox state** – Stalwart domain records and category mailbox metadata are synchronized into the module to keep list addressing and credentials aligned.
- **4: Outbound delivery** – The `sender` daemon claims pending deliveries, sends them over SMTP, and tracks transient failures for automatic retry.
- **5: Admin and member access** – The Dioxus UI reads scoped views from SpacetimeDB and updates state through reducer calls.

## Components

### SpacetimeDB Server (crate: `server`)
- Canonical database for accounts, categories, subscriptions, domains, and delivery state
- Reducers and HTTP handlers for user sync, MTA hooks, and admin workflows
- Scoped views that restrict data based on admin or member identity
- Temporary retry scheduling and delivery-lease recovery for SMTP resilience

### Admin Web Interface (crate: `admin`)
- Dioxus WebAssembly frontend for member and admin workflows
- OAuth login against Django and real-time updates from SpacetimeDB
- Management of subscriptions, categories, domains, and SMTP app-password metadata

### Sender Daemon (crate: `sender`)
- Claims pending fan-out work from SpacetimeDB
- Sends outbound mail through the configured SMTP path or relay
- Tracks retry attempts, lease expiry, and final delivery states

### External Dependencies
- **Django solawispielplatz**: identity provider, user sync, and OAuth flow
- **Stalwart MTA**: inbound mail processing, domain inventory, and mailbox provisioning
- **Outbound SMTP relay**: external mail delivery path when the system sends to non-local recipients

## Authentication Flow

```d2
{{#include architecture-auth-flow.d2}}
```

The system uses OAuth 2.0 with Django as the identity provider. JWT-bearing clients connect to SpacetimeDB through authenticated WebSocket and HTTP requests, while the module keeps admin-only views and reducer paths restricted to verified identities.
