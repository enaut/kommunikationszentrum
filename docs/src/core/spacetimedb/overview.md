# SpacetimeDB Server

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
