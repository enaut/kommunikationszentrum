# User Synchronisation

The user-sync endpoint allows the Django `solawispielplatz` backend to keep SpacetimeDB account data in sync with the canonical user store. Requests are authenticated with a bearer token carrying the `sync-user` permission.

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
    "categories": [
      {
        "name": "VP Reyerhof",
        "email_address": "vp-reyerhof@example.org",
        "description": "Verteilpunkt Reyerhof"
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
| `action` | ✓ | `"upsert"` to create-or-update the account; `"delete"` to deactivate it. |
| `user.mitgliedsnr` | ✓ | Canonical member ID from Django. |
| `user.name` | ✓ | Full display name. |
| `user.email` | ✓ | Primary email address. |
| `user.is_active` | ✓ | Account active flag. |
| `user.is_admin` | — | Grants admin privileges when `true`. |
| `user.updated_at` | — | ISO 8601 timestamp of last modification in Django. |
| `user.categories` | — | Mailing-list categories the account should be subscribed to. Each entry is created in `message_categories` if it doesn't already exist (matched by `email_address`); an existing category is never modified. Subscriptions are only ever **added** — omit or send `[]` to leave existing subscriptions untouched. |
| `user.unsubscribe_category_emails` | — | Email addresses of categories whose subscription should be deactivated for this account (the category row itself is never touched). Used when a member's Verteilpunkt changes: the old category is unsubscribed while `categories` adds the new one in the same request. |

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
