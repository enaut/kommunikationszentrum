# Event & Trigger Flow

This page describes the end-to-end data flows through the server module — from an email arriving
at the MTA to the final SMTP delivery to each subscriber, and from a user change in Django to an
updated account in SpacetimeDB.

---

## 1. Incoming Email Flow

### 1.1 MTA Hook Processing (SMTP session stages)

Every SMTP session triggers a sequence of hook calls from Stalwart to the `/mta-hook` HTTP
endpoint. The module responds synchronously to each stage with an `accept` or `reject` decision.

```d2
direction: right

internet: "Incoming\nEmail" {
  shape: oval
  style.fill: lightcoral
}
stalwart: "Stalwart MTA" {
  style.fill: lightgray
}
handler: "HTTP Handler\n(/mta-hook)" {
  style.fill: lightblue
}
db: "SpacetimeDB\nTables" {
  style.fill: lightyellow
}

internet -> stalwart: "SMTP session"

stalwart -> handler: "1. CONNECT\n(client IP)"
handler -> db: "check blocked_ips"
db -> handler: "blocked?"
handler -> stalwart: "accept / reject 550"

stalwart -> handler: "2. EHLO\n(HELO string)"
handler -> stalwart: "accept / reject 501"

stalwart -> handler: "3. MAIL FROM\n(sender address)"
handler -> stalwart: "accept / reject 550"

stalwart -> handler: "4. RCPT TO\n(recipient)"
handler -> db: "lookup message_categories"
db -> handler: "active category?"
handler -> stalwart: "accept / reject 550"

stalwart -> handler: "5. DATA\n(full message)"
handler -> db: "persist: received_message\n+ mail_ingress (pending)"
handler -> stalwart: "accept + X-Processed-By header"
```

### 1.2 DATA Stage Detail

The DATA stage is where the message is actually stored and the delivery pipeline is seeded.

```d2
direction: down

start: "DATA hook received" {
  shape: oval
  style.fill: lightcoral
}
resolve: "Resolve recipients\nto message_categories\n(envelope To -> header To fallback)"
sender: "Look up sender\naccount by email"
filter: "Filter categories:\nsender must be admin\nOR have active subscription"
no_cat: "quarantine\n(no authorized categories)" {
  shape: diamond
  style.fill: lightyellow
}
persist: "For each authorized category:\n• INSERT received_message\n• INSERT mail_ingress (pending)"
resp: "Return accept response\nto Stalwart" {
  shape: oval
  style.fill: lightgreen
}

start -> resolve
resolve -> sender
sender -> filter
filter -> no_cat: "none left"
filter -> persist: "≥1 category"
persist -> resp
```

**Key checks in the DATA stage:**
1. Resolve RCPT envelope addresses → `message_categories` (with `To`-header fallback).
2. Look up sender's `account` row by email.
3. Check if sender is in `admin_identities`.
4. For non-admin senders: filter out categories where no active `subscriptions` row exists for
   that (account, category) pair. External senders (not in `account`) are always rejected.
5. If any categories remain: insert `ReceivedMessage` + `MailIngress` (state = `pending`).

---

## 2. Delivery Pipeline Flow

After the MTA hook creates `MailIngress` records, the external **sender daemon** takes over.
The protocol is a claim/complete loop using SpacetimeDB reducers.

```d2
direction: down

pending: "MailIngress\n(state: pending)" { style.fill: lightyellow }
claim_i: "sender daemon calls\nclaim_next_mail_ingress" { style.fill: lightblue }
proc: "MailIngress\n(state: processing)" { style.fill: lightblue }

fanout: "Daemon fans out:\n• Query subscriptions for category\n• For each subscriber:\n  call enqueue_mail_delivery" { style.fill: lightblue }
queued: "MailDelivery rows\n(state: queued)" { style.fill: lightyellow }

claim_d: "sender daemon calls\nclaim_next_mail_delivery" { style.fill: lightblue }
sending: "MailDelivery\n(state: sending)" { style.fill: lightblue }

smtp: "Daemon submits\nSMTP to Stalwart" { style.fill: lightgray }

sent: "mark_mail_delivery_sent\n→ state: sent" { style.fill: lightgreen }
retry: "schedule_mail_delivery_retry\n→ state: retry_scheduled" { style.fill: lightyellow }
fail: "fail_mail_delivery\n→ state: failed" { style.fill: lightcoral }
bounce: "mark_mail_delivery_bounced\n→ state: bounced" { style.fill: lightcoral }

complete: "complete_mail_ingress\n→ state: completed" { style.fill: lightgreen }

pending -> claim_i: "daemon poll"
claim_i -> proc
proc -> fanout
fanout -> queued
queued -> claim_d: "daemon poll"
claim_d -> sending
sending -> smtp

smtp -> sent: "2xx"
smtp -> retry: "4xx / transient"
smtp -> fail: "5+ attempts"
smtp -> bounce: "5xx permanent"

retry -> queued: "back-off timer\n(30s → 2m → 10m → 30m → 60m → 12h)" { style.stroke-dash: 5 }
sent -> complete: "all deliveries done"
fail -> complete: "all deliveries terminal"
bounce -> complete: "all deliveries terminal"
```

### Lease & Claim Protocol

The sender daemon uses an **optimistic claim** pattern to coordinate work without a separate
queue broker:

1. Call `claim_next_mail_ingress` — atomically marks one `MailIngress` as `processing`,
   sets `claim_owner = sender_identity`, and sets `claim_expires_at = now + 10min`.
2. Subscribe to `sender_mail_ingress` view and observe the row changing to `processing` with
   `claim_owner == own_identity`.
3. Fan out deliveries, then call `complete_mail_ingress` (or `retry_mail_ingress` on error).
4. Repeat for deliveries via `claim_next_mail_delivery` (5-minute lease).

If the daemon crashes, the lease expires and another daemon instance can re-claim the work.

---

## 3. User Synchronization Flow

```d2
direction: right

django: "Django\nsolawispielplatz" { style.fill: lightyellow }
handler: "HTTP Handler\n(/user-sync)" { style.fill: lightblue }
auth: "Token auth check\n(sync-user permission)" {
  shape: diamond
  style.fill: white
}
do_sync: "do_sync_user()\nin transaction" { style.fill: lightblue }
account: "account table" { style.fill: lightyellow }
admin: "admin_identities\ntable" { style.fill: lightyellow }

django -> handler: "POST /user-sync\nBearer token + JSON"
handler -> auth
auth -> handler: "forbidden 403" { style.stroke: red; style.stroke-dash: 5 }
auth -> do_sync: "authorized"

do_sync -> account: "upsert or delete row"
do_sync -> admin: "grant/revoke admin\nbased on is_admin flag"
do_sync -> handler: "Ok(())"
handler -> django: "200 success"
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

## 4. One-Click Unsubscribe Flow

```d2
direction: right

client: "Email Client\n(RFC 8058 One-Click)" {
  shape: oval
  style.fill: lightcoral
}
handler: "HTTP Handler\n(/mailing-list/unsubscribe)" { style.fill: lightblue }
lookup: "Lookup token in\nsubscription_unsubscribe_tokens" { shape: diamond }
deact: "Set subscription.active = false\nSet token.active = false" { style.fill: lightblue }
resp: "200 {\"status\":\"unsubscribed\"}" {
  shape: oval
  style.fill: lightgreen
}

client -> handler: "POST ?token=<token>\nList-Unsubscribe=One-Click"
handler -> lookup
lookup -> handler: "404 not found" { style.stroke: red; style.stroke-dash: 5 }
lookup -> deact: "found"
deact -> resp
```

The token value is embedded by the sender daemon in the `List-Unsubscribe` and
`List-Unsubscribe-Post` headers of every outgoing delivery email.

---

## 5. WebSocket Subscription Model

The Admin UI and sender daemon connect to SpacetimeDB over WebSocket and subscribe to **views**,
not raw tables. SpacetimeDB pushes incremental row updates whenever a subscribed view's result
set changes.

```d2
direction: right

admin_ui: "Admin UI\n(Dioxus/WASM)" { style.fill: lightblue }
daemon: "Sender Daemon\n(Rust)" { style.fill: lightblue }
spacetime: "SpacetimeDB\nViews" { style.fill: lightcoral }

admin_ui -> spacetime: "subscribe:\nvisible_accounts\nvisible_subscriptions\nvisible_messages\nvisible_admin_identities\nvisible_webhook_tokens" { style.stroke-dash: 5 }
daemon -> spacetime: "subscribe:\nsender_mail_ingress\nsender_mail_deliveries" { style.stroke-dash: 5 }

spacetime -> admin_ui: "row updates (WebSocket)" { style.stroke: purple }
spacetime -> daemon: "row updates (WebSocket)" { style.stroke: purple }

admin_ui -> spacetime: "call reducers\n(e.g. add_subscription)" { style.stroke: green }
daemon -> spacetime: "call reducers\n(e.g. claim_next_mail_ingress)" { style.stroke: green }
```

**View selection by identity:**
- When an admin connects, `visible_accounts` returns all accounts.
- When a regular user connects, `visible_accounts` returns only their own row.
- The sender daemon connects with its own identity (which must be added to `admin_identities`)
  and subscribes to `sender_mail_ingress` / `sender_mail_deliveries`.

---

## 6. Category Provisioning Flow

```d2
direction: right

admin_ui: "Admin UI" {
  shape: oval
  style.fill: lightblue
}
procedure: "provision_message_category\n(Procedure)" { style.fill: lightblue }
auth: "Admin check\n+ duplicate check" { shape: diamond }
jmap: "Stalwart JMAP\nx:Account/set" { style.fill: lightgray }
insert: "INSERT message_categories\n(in transaction)" { style.fill: lightblue }
done: "Category live" {
  shape: oval
  style.fill: lightgreen
}

admin_ui -> procedure: "call via\nSpacetimeDB client"
procedure -> auth
auth -> procedure: "Err: unauthorized\nor duplicate" { style.stroke: red; style.stroke-dash: 5 }
auth -> jmap: "POST JMAP request"
jmap -> procedure: "notCreated?" { style.stroke: red; style.stroke-dash: 5 }
jmap -> insert: "created"
insert -> done
```
