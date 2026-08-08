# Event & Trigger Flow

This page describes the end-to-end data flows through the server module — from an email arriving
at the MTA to the final SMTP delivery to each subscriber, and from a user change in Django to an
updated account in SpacetimeDB.

---


---

## 3. User Synchronization Flow

```d2
{{#include event-flow-user-sync.d2}}
```

**Identity derivation:** The account's SpacetimeDB `Identity` is computed deterministically
from the Django OAuth issuer URL and the user's `mitgliedsnr`:

```rust
let issuer_url = format!("{}{}", DJANGO_OAUTH_BASE_URL, DJANGO_OAUTH_ISSUER_PATH);
let identity = Identity::from_claims(&issuer_url, &mitgliedsnr.to_string());
```

This means the identity stored in `account` will match the identity that the user's browser
presents when it connects via the Admin UI OAuth flow — no additional mapping is needed.

---


---

## 5. WebSocket Subscription Model

The Admin UI and sender daemon connect to SpacetimeDB over WebSocket and subscribe to **views**,
not raw tables. SpacetimeDB pushes incremental row updates whenever a subscribed view's result
set changes.

```d2
{{#include event-flow-websocket-subscription.d2}}
```

**View selection by identity:**
- When an admin connects, `visible_accounts` returns all accounts.
- When a regular user connects, `visible_accounts` returns only their own row.
- The sender daemon connects with its own identity (which must be added to `admin_identities`)
  and subscribes to `sender_mail_ingress` / `sender_mail_deliveries`.

---

## 6. Category Provisioning Flow

```d2
{{#include event-flow-category-provisioning.d2}}
```
