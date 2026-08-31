# Overview

The **Kommunikationszentrum** is a email management system designed for the SoLaWi (Solidarische Landwirtschaft / Community Supported Agriculture) project. It provides intelligent email routing and subscription management.

## What is the Kommunikationszentrum?

The Kommunikationszentrum acts as an intelligent email gateway that:

- **Filters and routes emails** based on mailing lists and user subscriptions
- **Manages user subscriptions** to different mailing lists
- **Integrates with existing systems** like Stalwart MTA and Django user management
- **Offers a web interface** for subscription and administrative management

```d2
{{#include overview-system.d2}}
```

## Key Features

### 🔐 **Authentication & Authorization**
- OAuth integration with the Django solawispielplatz identity provider
- JWT-based authorization for WebSocket and HTTP access
- Role-based admin checks enforced in SpacetimeDB views and reducers
- User synchronization from Django into the canonical subscription model

### 📧 **Mail routing & category management**
- Stalwart MTA hooks are normalized and classified in SpacetimeDB
- Category addresses are provisioned as mailing-list identities with visibility rules
- Domain metadata and mailbox state are synchronized from Stalwart into the module
- Account activation state is synchronized from Django and enforced before delivery is created
- Category-specific SMTP app passwords support list submission without broad credentials

### 🧯 **Delivery resilience**
- Transient SMTP errors are stored in a temporary retry queue instead of being lost
- Delivery jobs are leased to worker instances and recovered after lease expiry
- Sender state and final delivery outcomes are kept in the database for inspection

### 👥 **Member experience**
- Self-service subscription management in the Dioxus admin UI
- Admin controls for users, categories, domains, and routing state
- Real-time updates via SpacetimeDB subscriptions and scoped views

## Use Cases

The Kommunikationszentrum is designed for organizations that need:

1. **Mailing List Management**: Organizations with multiple email categories (news, events, announcements) where users can selectively subscribe
3. **User Integration**: Seamless integration with existing user management systems
5. **Self-Service**: Allow users to manage their own subscriptions without admin intervention

## Technology Stack

### **Backend Components**
- **SpacetimeDB**: Modern database with real-time capabilities and rust/WASM modules

### **Frontend Components**
- **Dioxus**: Rust-based WebAssembly frontend framework
- **WebAssembly**: Near-native performance in the browser
- **Bootstrap**: Responsive UI components with dioxus-bootstrap

### **Integration Technologies**
- **OAuth 2.0 / OpenID Connect**: Secure authentication
- **JWT**: Stateless authentication tokens
- **JSON**: Data exchange format
- **HTTP Webhooks**: Real-time event processing

## Architecture Principles

The Kommunikationszentrum follows these key principles:

### **Modularity**
Each component has a single responsibility:
- **SpacetimeDB Server**: Data storage and business logic
- **Admin Interface**: User interaction
- **Sender**: Email fan-out and sending
- **Django Integration**: User management and authentication

## Target Audience

### **End Users**
Community members who want to:
- Manage their email subscriptions
- Subscribe/unsubscribe from categories
- View their subscription status

### **Administrators**
System administrators who:
- Manage user accounts and permissions
- Configure email categories

### **Developers**
Technical team members who:
- Deploy and maintain the system
- Integrate with other services
- Extend functionality
- Debug issues

## Getting Started

For a quick start, see the [Quick Start Guide](../introduction/quick-start.md) which will get you up and running with a development environment in minutes.

For detailed setup instructions, proceed to the [Setup & Installation](../setup/prerequisites.md) section.

To understand the system architecture in detail, continue to the [Architecture](../core/architecture.md) chapter.
