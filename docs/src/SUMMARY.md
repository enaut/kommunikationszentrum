
# Kommunikationszentrum Documentation

# Kommunikationszentrum

- [Overview](./introduction/overview.md)
- [Quick Start Guide]() <!-- TODO: Complete when software is more mature -->

- [Setup & Installation](./setup/installation.md)
  - [Prerequisites](./setup/prerequisites.md)
  - [Development Environment](./setup/development-environment.md)
  - [Configuration](./setup/configuration.md)
  - [Component Installation](./setup/component-installation.md)
    - [SpacetimeDB Server](./setup/spacetimedb-server.md)
    - [Webhooks (Module HTTP Handlers)](./setup/spacetime-http-handlers.md)
    - [Admin Interface](./setup/admin-interface.md)
    - [Django Integration](./setup/django-integration.md)

- [Core Components](./core/architecture.md)

  - [SpacetimeDB Server](./core/spacetimedb/overview.md)
    - [Database Schema](./core/spacetimedb/database-schema.md)
    - [Reducers Reference](./core/spacetimedb/reducers-reference.md)
    - [Module Publishing](./core/spacetimedb/module-publishing.md)

    - [SpacetimeDB HTTP Handlers (API)](./core/spacetimedb/http-handlers/overview.md)
      - [MTA Hook Processing](./core/spacetimedb/http-handlers/mta-hook-processing.md)
      - [User Synchronization](./core/spacetimedb/http-handlers/user-sync.md)
      - [API Endpoints](./core/spacetimedb/http-handlers/api-endpoints.md)

  - [Sender Daemon](./core/sender/overview.md)
    - [Configuration](./core/sender/configuration.md)
    - [Control Flow](./core/sender/control-flow.md)
    - [Usage Guide](./core/sender/usage.md)

  - [Admin Interface](./core/admin/overview.md)
    - [User Guide](./core/admin/user-guide.md)
    - [Authentication](./core/admin/authentication.md)
    - [Subscription Management](./core/admin/subscription-management.md)
    - [Admin Features](./core/admin/admin-features.md)


- [Email System Integration](./email/overview.md)
  - [Stalwart MTA Setup](./email/stalwart-setup.md)
  - [MTA Hook Configuration](./email/mta-hook-config.md)
  - [Email Categories](./email/categories.md)
  - [Subscription System](./email/subscriptions.md)
  - [Processing Flow](./email/processing-flow.md)
  - [Trigger Flow](./email/flow-email-triggers.md)

- [Authentication & Security](./auth/overview.md)
  - [OAuth in Django](./auth/configure-django.md)
  - [OAuth Integration](./auth/oauth-integration.md)
  - [JWT Token Handling](./auth/jwt-tokens.md)
  - [User Permissions](./auth/permissions.md)

- [Development](./development/overview.md)
  - [Development Workflow](./development/workflow.md)
  - [Code Structure](./development/code-structure.md)
  - [Testing](./development/testing.md)
  - [Debugging](./development/debugging.md)
  - [Server Module](./development/server.md)
  - [Sender Daemon](./development/sender.md)
  - [Admin Frontend Restructure](./development/admin-frontend-restructure.md)

- [Operations](./operations/overview.md)
  - [Deployment]()
  - [Monitoring]()
  - [Backup & Recovery]()
  - [Troubleshooting]()

- [Reference](./reference/overview.md)
  - [Environment Variables](./reference/environment-variables.md)
  - [Command Line Tools]()
  - [Configuration Files]()
  - [Error Codes]()
  - [FAQ]()

- [Appendices](./appendices/overview.md)
  - [Database Diagrams](./appendices/database-diagrams.md)
  - [Glossary]()
  - [Migration Guides](./appendices/migrating-imap.md)
