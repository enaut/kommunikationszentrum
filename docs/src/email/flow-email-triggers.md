# Email Trigger Flow

### Inbound message processing

1. Stalwart sends SMTP-stage hook requests to the SpacetimeDB `/mta-hook` endpoint.
2. The DATA-stage handler validates topic recipients, sender identity, permissions, and message size.
3. Accepted topic messages are stored as canonical `MailMessage` records with `MailIngress` state. Rejected topics create `ReceivedMessage` outcomes and may queue a rejection notification in `system_mail_pending`.
4. The scheduled JMAP dispatcher expands accepted ingress into recipient-specific delivery rows and routes queued system mail through the same delivery path.

### Scheduled outbound delivery

The SpacetimeDB module's scheduled procedure claims due JMAP delivery records and submits each recipient-specific message through Stalwart's JMAP API. Delivery state and audit events are persisted in the module; transient or uncertain results are retried, while permanent failures are visible in the Admin Status page. Dispatch is disabled unless `JMAP_DISPATCH_ENABLED=true`.

See [Outbound Delivery](../core/outbound-delivery.md) for the delivery states, migration behavior, and configuration.

### One-click unsubscribe

The module includes RFC 8058 `List-Unsubscribe` and `List-Unsubscribe-Post` headers in outgoing mailing-list messages. A member can unsubscribe using the token-backed HTTP endpoint or the Admin interface.
