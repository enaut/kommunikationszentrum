# Database Schema

The SpacetimeDB database for Kommunikationszentrum manages identity and access control, mailing list categories and subscriptions, MTA hook audit logging, and asynchronous mail delivery fan-out.

## Full Entity-Relationship Schema

Tables marked **`[Public]`** are client-queryable (subject to row-level filters/views), while **`[Private]`** tables are restricted to server-side reducers and HTTP handlers.

```d2
direction: down

"User Management & Auth": {
  style.fill: "#eef2f7"

  account: {
    shape: sql_table
    label: "account [Public]"
    id: "u64 (PK)"
    identity: "Identity (UQ)"
    email: "String (BTree)"
    name: "String"
    is_active: "bool"
    last_synced: "Timestamp (BTree)"
  }

  admin_identities: {
    shape: sql_table
    label: "admin_identities [Private]"
    identity: "Identity (PK)"
  }

  webhook_tokens: {
    shape: sql_table
    label: "webhook_tokens [Private]"
    id: "u64 (PK, auto_inc)"
    token_hash: "String (UQ)"
    "label": "String"
    permissions: "Vec<String>"
    created_at: "Timestamp (BTree)"
    active: "bool"
  }
}

"Mailing & Subscriptions": {
  style.fill: "#e8f5e9"

  message_categories: {
    shape: sql_table
    label: "message_categories [Public]"
    id: "u64 (PK, auto_inc)"
    email_address: "String (UQ)"
    name: "String"
    description: "String"
    active: "bool"
  }

  subscriptions: {
    shape: sql_table
    label: "subscriptions [Public]"
    id: "u64 (PK, auto_inc)"
    subscriber_account_id: "u64 (BTree)"
    subscriber_email: "String (BTree)"
    category_id: "u64 (BTree)"
    subscribed_at: "Timestamp"
    status: "SubscriptionStatus (BTree)"
  }

  subscription_unsubscribe_tokens: {
    shape: sql_table
    label: "subscription_unsubscribe_tokens [Public]"
    token: "String (PK)"
    subscription_id: "u64 (UQ)"
    created_at: "Timestamp (BTree)"
    active: "bool"
    revoked_at: "Timestamp"
  }
}

"MTA Audit & Processing": {
  style.fill: "#fff3e0"

  mta_connection_log: {
    shape: sql_table
    label: "mta_connection_log [Private]"
    id: "u64 (PK, auto_inc)"
    client_ip: "String"
    stage: "String"
    action: "String"
    timestamp: "Timestamp"
    details: "String"
  }

  mta_message_log: {
    shape: sql_table
    label: "mta_message_log [Private]"
    id: "u64 (PK, auto_inc)"
    from_address: "String"
    to_addresses: "String (JSON)"
    subject: "String"
    message_size: "u64"
    stage: "String"
    action: "String"
    timestamp: "Timestamp"
    queue_id: "Option<String>"
  }

  received_message: {
    shape: sql_table
    label: "received_message [Private]"
    id: "u64 (PK, auto_inc)"
    queue_id: "Option<String>"
    received_at: "Timestamp (BTree)"
    sender_account_id: "Option<u64>"
    sender_email: "String"
    category_id: "u64 (BTree)"
    category_email: "String"
    subject: "String"
    headers_raw: "String (JSON)"
    body_raw: "String"
    message_size: "u64"
  }

  blocked_ips: {
    shape: sql_table
    label: "blocked_ips [Private]"
    ip: "String (PK)"
    reason: "String"
    blocked_at: "Timestamp"
    active: "bool"
  }
}

"Delivery Pipeline": {
  style.fill: "#f3e5f5"

  mail_ingress: {
    shape: sql_table
    label: "mail_ingress [Public]"
    id: "String (PK)"
    queue_id: "String (BTree)"
    category_id: "u64 (BTree)"
    state: "String (BTree)"
    next_attempt_at: "Timestamp (BTree)"
    received_at: "Timestamp (BTree)"
    sender_account_id: "Option<u64>"
    sender_email: "String"
    category_email: "String"
    claim_owner: "Option<Identity>"
    claim_expires_at: "Timestamp"
    attempt_count: "u32"
    recipient_count: "u32"
    delivery_count: "u32"
    failed_delivery_count: "u32"
  }

  mail_deliveries: {
    shape: sql_table
    label: "mail_deliveries [Public]"
    id: "String (PK)"
    ingress_id: "String (BTree)"
    category_id: "u64 (BTree)"
    subscription_id: "u64 (BTree)"
    recipient_email: "String (BTree)"
    state: "String (BTree)"
    next_attempt_at: "Timestamp (BTree)"
    recipient_account_id: "Option<u64>"
    unsubscribe_token: "String"
    claim_owner: "Option<Identity>"
    claim_expires_at: "Timestamp"
    attempt_count: "u32"
    sent_at: "Timestamp"
  }

  mail_delivery_events: {
    shape: sql_table
    label: "mail_delivery_events [Public]"
    id: "u64 (PK, auto_inc)"
    delivery_id: "String (BTree)"
    occurred_at: "Timestamp (BTree)"
    event_type: "String"
    attempt_no: "u32"
    smtp_status_code: "Option<u16>"
    error_kind: "Option<String>"
    worker_identity: "Option<Identity>"
  }
}

# Key Foreign Keys & Relationships
"User Management & Auth".admin_identities -> "User Management & Auth".account: "identity → identity"

"Mailing & Subscriptions".subscriptions -> "User Management & Auth".account: "subscriber_account_id → id"
"Mailing & Subscriptions".subscriptions -> "Mailing & Subscriptions".message_categories: "category_id → id"
"Mailing & Subscriptions".subscription_unsubscribe_tokens -> "Mailing & Subscriptions".subscriptions: "subscription_id → id"

"Delivery Pipeline".mail_ingress -> "Mailing & Subscriptions".message_categories: "category_id → id"
"Delivery Pipeline".mail_ingress -> "User Management & Auth".account: "sender_account_id → id"
"Delivery Pipeline".mail_deliveries -> "Delivery Pipeline".mail_ingress: "ingress_id → id"
"Delivery Pipeline".mail_deliveries -> "Mailing & Subscriptions".message_categories: "category_id → id"
"Delivery Pipeline".mail_deliveries -> "Mailing & Subscriptions".subscriptions: "subscription_id → id"
"Delivery Pipeline".mail_delivery_events -> "Delivery Pipeline".mail_deliveries: "delivery_id → id"

"MTA Audit & Processing".received_message -> "User Management & Auth".account: "sender_account_id → id"
"MTA Audit & Processing".received_message -> "Mailing & Subscriptions".message_categories: "category_id → id"
```

---

## Delivery Pipeline Architecture

The delivery pipeline decouples MTA message acceptance from subscriber fan-out and outbound SMTP delivery using lease-based worker scheduling.

```d2
direction: right

MTA: {
  shape: cloud
  label: "Stalwart MTA / Webhook"
}

Ingress: {
  shape: class
  label: "mail_ingress [Public]\n(1 per category email)"
  state: "pending | processing | retry_scheduled | completed | failed"
  lease: "claim_owner & claim_expires_at (10 min)"
}

FanOut: {
  shape: diamond
  label: "Fan-Out Engine\n(Sender Daemon)"
}

Deliveries: {
  shape: class
  label: "mail_deliveries [Public]\n(1 per active subscriber)"
  state: "queued | sending | retry_scheduled | sent | failed | bounced"
  lease: "claim_owner & claim_expires_at (5 min)"
}

Audit: {
  shape: page
  label: "mail_delivery_events [Public]\n(Immutable audit trail)"
}

MTA -> Ingress: "POST /mta-hook (DATA stage)"
Ingress -> FanOut: "Claim ingress lease"
FanOut -> Deliveries: "Generate subscriber deliveries"
Deliveries -> Audit: "Log attempt & SMTP response"
```

---

## Subscription Lifecycle & Unsubscribe Flow

Subscriptions track user opt-in status across automated Django syncs and manual user actions.

```d2
direction: down

"Sync Path (Django Webhook)": {
  AutomaticallySubscribed: {
    shape: oval
    style.fill: "#c8e6c9"
  }

  AutomaticallyUnsubscribed: {
    shape: oval
    style.fill: "#ffcdd2"
  }
}

"Explicit User / Admin Actions": {
  ManuallySubscribed: {
    shape: oval
    style.fill: "#a5d6a7"
  }

  ManuallyUnsubscribed: {
    shape: oval
    style.fill: "#ef9a9a"
  }

  LinkUnsubscribed: {
    shape: oval
    style.fill: "#ffcc80"
  }
}

"Sync Path (Django Webhook)".AutomaticallySubscribed -> "Sync Path (Django Webhook)".AutomaticallyUnsubscribed: "Sync update (if unchanged manually)"
"Sync Path (Django Webhook)".AutomaticallySubscribed -> "Explicit User / Admin Actions".LinkUnsubscribed: "List-Unsubscribe HTTP POST"
"Sync Path (Django Webhook)".AutomaticallySubscribed -> "Explicit User / Admin Actions".ManuallyUnsubscribed: "User/Admin UI toggle"
"Explicit User / Admin Actions".ManuallyUnsubscribed -> "Explicit User / Admin Actions".ManuallySubscribed: "Manual resubscription"
```

> **Sync Protection Rule:** Subscriptions in `ManuallySubscribed`, `ManuallyUnsubscribed`, or `LinkUnsubscribed` state are protected and will **never** be overwritten by automated Django background syncs.

---

## Table Access & Client Visibility Views

SpacetimeDB table access is split into public and module-private tables. Client visibility is controlled via computed views:

```d2
direction: right

"Private Tables": {
  admin_identities
  webhook_tokens
  mta_connection_log
  mta_message_log
  received_message
  blocked_ips
}

"Public Tables": {
  account
  message_categories
  subscriptions
  subscription_unsubscribe_tokens
  mail_ingress
  mail_deliveries
  mail_delivery_events
}

"Client Views": {
  visible_accounts: "Own account (User) / All (Admin)"
  visible_admin_identities: "Empty (User) / All (Admin)"
  visible_webhook_tokens: "Empty (User) / All (Admin)"
  visible_subscriptions: "Own subscriptions (User) / All (Admin)"
  active_subscriptions: "Active subscriptions only"
  active_unsubscribe_tokens: "Active tokens only"
  visible_messages: "Subscribed category messages (User) / All (Admin)"
  sender_mail_ingress: "Claimable/active ingress (Sender Daemon)"
  sender_mail_deliveries: "Claimable/active deliveries (Sender Daemon)"
}

"Private Tables" -> "Client Views": "Selective view projections"
"Public Tables" -> "Client Views": "Filtered subscriber projections"
```
