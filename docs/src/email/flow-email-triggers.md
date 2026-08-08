## 1. Incoming Email Flow

### 1.1 MTA Hook Processing (SMTP session stages)

Every SMTP session triggers a sequence of hook calls from Stalwart to the `/mta-hook` HTTP
endpoint. The module responds synchronously to each stage with an `accept` or `reject` decision.

```d2
{{#include event-flow-mta-hook.d2}}
```

### 1.2 DATA Stage Detail

The DATA stage is where the message is actually stored and the delivery pipeline is seeded.

```d2
{{#include event-flow-data-stage.d2}}
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
{{#include event-flow-delivery-pipeline.d2}}
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

## 4. One-Click Unsubscribe Flow

```d2
{{#include event-flow-unsubscribe.d2}}
```

The token value is embedded by the sender daemon in the `List-Unsubscribe` and
`List-Unsubscribe-Post` headers of every outgoing delivery email.
