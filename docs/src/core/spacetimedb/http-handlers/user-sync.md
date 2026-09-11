# User Synchronisation

The user-sync endpoint allows the Django `solawispielplatz` backend to keep SpacetimeDB account data in sync with the canonical user store. Requests are authenticated with a bearer token carrying the `sync-user` permission (see [Managing Webhook Tokens / Token Generation](../module-publishing.md#managing-webhook-tokens)).

---

## Authentication

Requests must include a bearer token in the `Authorization` header:

```http
Authorization: Bearer <token>
```

The token must carry the `sync-user` permission. For step-by-step instructions on generating a token, hashing it with BLAKE3, and registering it with SpacetimeDB, see [Managing Webhook Tokens / Token Generation](../module-publishing.md#managing-webhook-tokens).

In the Django backend, configure the plaintext token in `settings_local.py`:

```python
SPACETIME_WEBHOOK_TOKEN = "<plaintext-token>"
SPACETIME_SYNC_URL = "http://localhost:3000/v1/database/kommunikation/route/user-sync"
```

---

## Request Format

**Endpoint:** `POST /v1/database/kommunikation/route/user-sync`

```json
{
  "action": "upsert" | "delete",
  "user": {
    "mitgliedsnr": 12345,
    "name": "Full Name",
    "email": "user@example.org",
    "is_active": true,
    "is_admin": false,
    "updated_at": "2024-01-01T12:00:00Z",
    "account_emails": ["alt1@example.org", "alt2@example.org"],
    "categories": [
      {
        "name": "VP Reyerhof",
        "email_address": "vp-reyerhof@example.org",
        "description": "Verteilpunkt Reyerhof",
        "required": true
      }
    ],
    "unsubscribe_category_emails": ["vp-old@example.org"]
  }
}
```

---

## Field Reference

| Field | Required | Description |
|---|---|---|
| `action` | ✓ | `"upsert"` to create-or-update the account; `"delete"` to cascade-delete it. |
| `user.mitgliedsnr` | ✓ | Canonical member ID from Django. |
| `user.name` | ✓ | Full display name. |
| `user.email` | ✓ | Primary email address. Synchronized as verified `DjangoSync` email. |
| `user.is_active` | ✓ | Account active flag. |
| `user.is_admin` | — | Grants admin privileges when `true`. |
| `user.updated_at` | — | ISO 8601 timestamp of last modification in Django. |
| `user.account_emails` | — | Array of alternative email addresses synchronized from Django. |
| `user.categories` | — | Mailing-list categories the account should be subscribed to. Each entry is created in `message_categories` if missing. Subscriptions are created or activated. |
| `user.unsubscribe_category_emails` | — | Email addresses of categories whose subscription should be deactivated for this account. Deactivates all active subscriptions of that account for the category. |

---

## Multi-Email Reconciliation & Cascading Deletions

### Upsert & Email Reconciliation
1. **Primary Email Resolution**: Resolves or creates the primary email in `account_emails` with `source = EmailSource::DjangoSync` and `is_verified = true`.
2. **Alternative Emails**: Synchronizes any emails in `account_emails` payload under `DjangoSync`.
3. **Removed Emails**: When a previously synced email is no longer present in the sync payload:
   - Identifies subscriptions tied to that removed address.
   - If the user already has a subscription to that category on `primary_email_id`, the duplicate subscription and its unsubscribe token are safely deleted.
   - If no subscription on `primary_email_id` exists (e.g. primary address changed), the subscription is migrated to `primary_email_id`.
   - The obsolete `AccountEmail` row is deleted.

### Cascading Deletion (`action = "delete"`)
When an account is deleted from Django:
- All subscriptions and their corresponding `SubscriptionUnsubscribeToken` rows are deleted.
- All linked `AccountEmail` records are deleted.
- All pending `EmailVerificationToken` rows for the account are deleted.
- The `Account` row and any associated `AdminIdentity` records are removed.

---

## Response

| Status | Body |
|---|---|
| `200 OK` | `{ "status": "success", "action": "upsert", "mitgliedsnr": 12345 }` |
| `4xx` | Client errors (missing/invalid token, malformed JSON). |
| `5xx` | Server errors. |

---

## Retry Behaviour

If a sync request fails due to temporary network or server errors, the Django sender code queues the payload for retry. See `mitgliederverwaltung/signals.py` for the retry queue implementation.
