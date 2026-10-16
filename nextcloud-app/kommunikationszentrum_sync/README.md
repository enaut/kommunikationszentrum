# Kommunikationszentrum Nextcloud Sync App (`kommunikationszentrum_sync`)

A minimal Nextcloud app that hooks into Nextcloud's user lifecycle events (`UserCreatedEvent`, `UserChangedEvent`, `UserDeletedEvent`) and dispatches webhook notifications to the `nextcloud-sync` service.

## Installation

1. Copy or symlink this folder into your Nextcloud `apps` directory:
   ```bash
   cp -r /path/to/kommunikationszentrum/nextcloud-app/kommunikationszentrum_sync /var/www/nextcloud/apps/
   chown -R www-data:www-data /var/www/nextcloud/apps/kommunikationszentrum_sync
   # On Fedora / RHEL (SELinux):
   restorecon -Rv /var/www/nextcloud/apps/kommunikationszentrum_sync
   ```

2. Enable the app via `occ`:
   ```bash
   occ app:enable kommunikationszentrum_sync
   ```

## Configuration

Configure the webhook endpoint and authentication token:

```bash
occ config:app:set kommunikationszentrum_sync webhook_url --value="http://127.0.0.1:8088/nextcloud/webhook"
occ config:app:set kommunikationszentrum_sync webhook_secret --value="<your-NC_WEBHOOK_SECRET>"
```

## How It Works

- When a user is created, updated, or deleted in Nextcloud, this app sends a lightweight HTTP POST request to the configured `webhook_url` with the `X-Nextcloud-Token` header.
- The `nextcloud-sync` daemon receives the notification, fetches full user details from the Nextcloud OCS API, and upserts or deletes the user in SpacetimeDB with retries and exponential backoff.
- The app utilizes non-blocking short timeouts (2s connect, 3s request) and wraps calls in `try/catch` so Nextcloud user operations are never blocked or broken if the sync service is temporarily unavailable.

### SELinux (Fedora / RHEL)

- **File contexts**: Run `restorecon -Rv /var/www/nextcloud/apps/kommunikationszentrum_sync` after copying or updating files.
- **Outbound HTTP**: If Nextcloud cannot connect to `nextcloud-sync`, enable the SELinux boolean:
  ```bash
  sudo setsebool -P httpd_can_network_connect 1
  ```
