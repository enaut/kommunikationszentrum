# Configuration

Kommunikationszentrum uses build-time application profiles, SpacetimeDB environment values, and database-backed Stalwart settings.

### Application profiles

The Admin frontend and SpacetimeDB module use a profile from `profiles/<name>/`. Profiles contain backend URLs, OIDC settings, branding, and localized copy.

1. Copy the example profile: `cp -r profiles/example profiles/my-org`.
2. Adjust `profile.toml` and the profile locales.
3. Select the profile when serving or publishing:

```bash
APP_PROFILE=my-org dx serve --package admin --platform web
APP_PROFILE=my-org spacetime publish --project-path server kommunikation
```

Custom profiles other than `example/` are gitignored. See the [Configuration Profiles Reference](../reference/configuration.md) for supported fields.

### SpacetimeDB environment values

The scheduled JMAP dispatcher reads `JMAP_DISPATCH_ENABLED`; dispatch remains disabled unless its value is `true`. System mail uses the typed module environment values `SYSTEM_JMAP_USERNAME` and `SYSTEM_JMAP_PASSWORD`. Supply credentials through the deployment's SpacetimeDB environment configuration and keep them out of source control, delivery rows, and client-visible views.

Topic sends use the topic's app password stored in SpacetimeDB. The Stalwart JMAP URL and admin API token are managed through the `stalwart_config` table using the Admin UI or `set_stalwart_config`.

See the [environment variable reference](../reference/environment-variables.md) and [Outbound Delivery](../core/outbound-delivery.md) for the module's JMAP settings and dispatch behavior.

### Security and troubleshooting

- Keep production credentials out of tracked files and client-visible data.
- Use HTTPS for deployed SpacetimeDB, OIDC, and Stalwart endpoints.
- If delivery is not running, confirm `JMAP_DISPATCH_ENABLED`, the configured JMAP endpoint, module credentials, and scheduled-procedure status before changing delivery rows.
