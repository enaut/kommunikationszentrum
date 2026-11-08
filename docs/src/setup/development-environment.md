# Development Environment

This guide describes the local services used to develop Kommunikationszentrum.

### Components

1. **SpacetimeDB** (`:3000`): database and execution engine for reducers, procedures, scheduled jobs, and HTTP routes.
2. **Admin Web App** (`:8080`): Dioxus frontend for topics, categories, subscriptions, identities, webhook tokens, and delivery status.
3. **OIDC Provider** (`:8000` / `:443`): Django `solawispielplatz`, Nextcloud, or Keycloak. Login can provision an account on first connection.
4. **Stalwart** (`:8093` / `:25`): local MTA for inbound hooks, mailbox management, and JMAP.
5. **Observability stack**: optional Grafana, Loki, Tempo, and Alloy services in `grafana/`.

### Environment setup

Configure the application profile in `profiles/<name>/profile.toml` and the matching profile locales. Create local Stalwart settings from the tracked examples under `.env/` as needed. Keep credentials in ignored local configuration or the SpacetimeDB environment, not in source control.

### Running development services

Use the Zed tasks or run the commands individually:

```bash
spacetime start
spacetime publish --project-path server kommunikation
dx serve --package admin --platform web
mdbook serve docs --port 3022
```

The JMAP dispatcher is disabled unless `JMAP_DISPATCH_ENABLED=true` is configured for the SpacetimeDB module. Keep it disabled until the Stalwart JMAP credentials and the current delivery queue state have been checked. See [Outbound Delivery](../core/outbound-delivery.md).
