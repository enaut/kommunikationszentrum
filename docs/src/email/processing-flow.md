# Processing Flow

The email processing flow in the Kommunikationszentrum follows a multi-stage validation process based on MTA hooks from Stalwart.

## MTA Processing Pipeline

```d2
{{#include processing-pipeline.d2}}
```

## Processing Stages

### 1. CONNECT Stage
- **Purpose**: Initial connection validation
- **Checks**: IP blocking via `blocked_ips` table
- **Logging**: `mta_connection_log`
- **Actions**: ACCEPT or REJECT based on IP status

### 2. EHLO/HELO Stage  
- **Purpose**: Protocol compliance validation
- **Checks**: Basic HELO/EHLO syntax
- **Logging**: `mta_connection_log`
- **Actions**: ACCEPT for valid, REJECT for invalid

### 3. MAIL FROM Stage
- **Purpose**: Sender validation
- **Checks**: Email address format validation
- **Logging**: `mta_connection_log`
- **Actions**: ACCEPT for valid format, REJECT for invalid

### 4. RCPT TO Stage
- **Purpose**: Recipient and category validation
- **Checks**: 
  - Email address format
  - Category exists in `message_categories`
  - Category is active
- **Logging**: `mta_connection_log`
- **Actions**: ACCEPT for valid categories, REJECT for unknown

### 5. DATA Stage
- **Purpose**: Full message processing, sender account resolution, and permission validation
- **Checks**:
  - Sender address is looked up across `account_emails` (supports shared addresses across multiple accounts)
  - Admin privileges: sender is authorized if *any* matching account has an admin identity
  - Write permissions: for regular members, verifies that the account is active (`is_active == true`) and holds an active subscription with `SubscriptionPermission::Write` for the category
- **Logging**: `mta_message_log` (detailed message information)
- **Actions**: 
  - ACCEPT: Admin sender or member with active `Write` subscription; persists `mail_message`, archives `received_message`, and queues `mail_ingress` fan-out
  - QUARANTINE: Sender is unauthenticated, not an active member, or lacks `Write` permissions
  - REJECT: System errors or malformed payloads

### 6. AUTH Stage
- **Purpose**: Authentication handling
- **Current Implementation**: Accept-all (placeholder)
- **Logging**: `mta_connection_log`
- **Future**: Could integrate with Django authentication

## Decision Logic

### IP Blocking (CONNECT)
```rust
if blocked_ip.active && blocked_ip.ip == client_ip {
    return REJECT;
}
```

### Category Validation (RCPT)
```rust
if !message_categories.contains(recipient_email) || !category.active {
    return REJECT;
}
```

### Sender Authorization (DATA)
```rust
// 1. Resolve all active accounts associated with the sender email address
let sender_account_ids: Vec<u64> = ctx
    .db
    .account_emails()
    .email()
    .filter(&from_address.to_string())
    .map(|ae| ae.account_id)
    .filter(|acc_id| {
        ctx.db.account().id().find(acc_id).map_or(false, |acc| acc.is_active)
    })
    .collect();

// 2. Administrators are authorized to post to any valid category
let sender_is_admin = sender_account_ids.iter().any(|id| {
    ctx.db.account().id().find(id).map_or(false, |acc| {
        ctx.db.admin_identities().identity().find(&acc.identity).is_some()
    })
});

// 3. For non-admins, check subscriptions and permissions per category:
//    - If sender has Write permission: deliver to category
//    - If sender has Read-only permission: reject (NoWritePermission)
//    - If sender is not subscribed: reject (NotSubscribed)
//    - If sender email is not registered/active: reject (NotRegistered)
//
// 4. For any rejected categories, an explanatory rejection email is queued
//    in `system_mail_pending` and dispatched by the sender daemon via SMTP_SYSTEM_USER.
```