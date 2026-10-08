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
    "external_id": "12345",
    "name": "Full Name",
    "is_active": true,
    "is_admin": false,
    "emails": [
      {
        "email": "user@example.org",
        "is_primary": true,
        "is_verified": true
      },
      {
        "email": "alt1@example.org",
        "is_primary": false,
        "is_verified": true
      }
    ],
    "topics": [
      {
        "name": "VP Reyerhof",
        "email_address": "vp-reyerhof@example.org",
        "description": "Verteilpunkt Reyerhof",
        "categories": ["Verteilpunkt"],
        "required": true,
        "default_permission": "write"
      }
    ],
    "unsubscribe_topic_emails": ["vp-old@example.org"]
  }
}
```

---

## Field Reference

| Field | Required | Description |
|---|---|---|
| `action` | ✓ | `"upsert"` to create-or-update the account; `"delete"` to cascade-delete it. |
| `user.external_id` | ✓ | Canonical subject/member ID from the external system (matches OIDC `sub` claim). |
| `user.name` | ✓ | Full display name. |
| `user.is_active` | ✓ | Account active flag. |
| `user.is_admin` | — | Grants admin privileges when `true`, revokes when `false`. If omitted or `null`, existing admin privileges remain untouched. |
| `user.emails` | ✓ (upsert) | Array of email objects (`email`, `is_primary`, `is_verified`). Upsert requires exactly one email with `is_primary: true`. Synchronized under `EmailSource::ExternalSync` with the declared `is_verified` status. |
| `user.topics` | — | Mailing-list topics the account should be subscribed to. Each entry is created in `message_topics` if missing. Subscriptions are created or activated. May specify `categories` (e.g. `["Verteilpunkt"]`) and `default_permission` (`"read"` or `"write"`). |
| `user.unsubscribe_topic_emails` | — | Email addresses of topics whose subscription should be deactivated for this account. Deactivates all active subscriptions of that account for the topic. |

---

## Multi-Email Reconciliation & Cascading Deletions

### Upsert & Email Reconciliation
1. **Email Synchronization & Verification**: Resolves or creates all entries in `emails` with `source = EmailSource::ExternalSync` and their authoritative `is_verified` status (updating existing rows if verification changed).
2. **Primary Email Assignment**: The address with `is_primary = true` is assigned as the account's `primary_email_id`.
3. **Removed Emails**: When a previously synced `ExternalSync` email is no longer present in the `emails` array:
   - Identifies subscriptions tied to that removed address.
   - If the user already has a subscription to that topic on `primary_email_id`, the duplicate subscription and its unsubscribe token are safely deleted.
   - If no subscription on `primary_email_id` exists (e.g. primary address changed), the subscription is migrated to `primary_email_id`.
   - The obsolete `AccountEmail` row is deleted.

### Cascading Deletion (`action = "delete"`)
When an account is deleted from the external system:
- All subscriptions and their corresponding `SubscriptionUnsubscribeToken` rows are deleted.
- All linked `AccountEmail` records are deleted.
- All pending `EmailVerificationToken` rows for the account are deleted.
- The `Account` row and any associated `AdminIdentity` records are removed.

---

## Response

| Status | Body |
|---|---|
| `200 OK` | `{ "status": "success", "action": "upsert", "external_id": "12345" }` |
| `4xx` | Client errors (missing/invalid token, malformed JSON). |
| `5xx` | Server errors. |

---

## Retry Behaviour

If a sync request fails due to temporary network or server errors, the Django sender code queues the payload for retry. See `mitgliederverwaltung/signals.py` for the retry queue implementation.
