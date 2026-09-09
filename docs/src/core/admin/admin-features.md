# Admin Features

The Kommunikationszentrum Admin Web Interface provides administrative control cards for database security, external integrations, webhook credentials, and domain synchronization.

---

## 1. Admin Identity Management

Admin identities have permission to invoke administrative reducers and procedures in SpacetimeDB. The **Admin-Identitäten** card allows managing authorized administrators:

![Admin Identities Management](../../email/img/admin-identity-management.png)

- **Granting Admin Privileges**:
  1. Enter the target user's 64-character hexadecimal SpacetimeDB Identity in the **Identity Hex (64 Zeichen)** field.
  2. Click **+ Hinzufügen** to invoke `register_admin_identity`.
- **Revoking Admin Privileges**:
  - Click the remove icon next to any listed identity to invoke `unregister_admin_identity`.
- **Identity Count**:
  - The badge shows the current number of active admin identities in the `admin_identities` table.

---

## 2. Webhook Token Management

External systems (Stalwart MTA and the Django backend) authenticate to SpacetimeDB HTTP routes using bearer webhook tokens. Tokens are managed client-side for maximum security:

![Webhook Token Management](../../email/img/admin-mta-token-creation.png)

- **Creating a Token**:
  1. Enter a **Label** (e.g. `Stalwart` or `Sync User Token`).
  2. Select the required **Permission**:
     - `mta-hook`: For Stalwart MTA webhook routes (`POST /mta-hook`).
     - `sync-user`: For Django user account synchronization (`POST /user-sync`).
  3. Click **+ Token generieren** to produce a secure random 32-byte token in the browser.
  4. Click **Kopieren** to store the plaintext secret safely.
  5. Click **Token erstellen** to register the token. The browser calculates the BLAKE3 hash locally and passes only the hash to `create_webhook_token`.
- **Revoking a Token**:
  - Click the trash icon next to any registered token to call `revoke_webhook_token`.

> [!NOTE]
> The server stores only the BLAKE3 cryptographic hash. Plaintext tokens are shown once in the browser and can never be retrieved from the server.

---

## 3. Stalwart Mailserver Configuration

To enable automated mailbox creation and domain synchronization, the SpacetimeDB module requires credentials to Stalwart's JMAP REST API:

![Stalwart Mailserver Configuration](../../email/img/admin-jmap-url-credentials.png)

- **Configuration Fields**:
  - **JMAP-URL**: The base URL of Stalwart's JMAP endpoint (typically `http://<host>:8093/jmap` or `https://<domain>/jmap`).
  - **Admin-Token**: The secret API key configured in Stalwart with admin privileges (`STALWART_ADMIN_TOKEN`; see [Stalwart MTA Setup](../../email/stalwart-setup.md#admin-api-key-configuration-stalwart_admin_token)).
- **Saving**:
  - Click **Speichern** to call the `set_stalwart_config` reducer and persist the settings into the `stalwart_config` table.
- **Status Indicator**:
  - Displays `Konfiguriert` when active credentials and an endpoint are saved, along with the timestamp of the last update.

---

## 4. Domain Management & Synchronization

Mailing list categories must belong to an active domain configured in the mail server. The **Domains** card synchronizes domains directly from Stalwart:

![Domain Synchronization](../../email/img/admin-url-sync.png)

- **Synchronizing Domains**:
  - Click **Jetzt synchronisieren** to invoke the `sync_stalwart_domains` procedure.
  - SpacetimeDB contacts Stalwart's JMAP API, queries all configured domains, and updates the `domains` table.
- **Domain List**:
  - Displays all active domains (e.g. `solawis.de`) with their internal IDs and descriptions.
  - Newly synchronized domains immediately become selectable in the category creation interface.
