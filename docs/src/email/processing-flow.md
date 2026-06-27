# Processing Flow

The email processing flow in the Kommunikationszentrum follows a multi-stage validation process based on MTA hooks from Stalwart.

## MTA Processing Pipeline

```d2
direction: right

stages: "MTA Hook Stages" {
  style.fill: "#e0f7fa"
  connect: "CONNECT\nStage"
  ehlo: "EHLO\nStage"
  mail: "MAIL FROM\nStage"
  rcpt: "RCPT TO\nStage"
  data: "DATA\nStage"
  auth: "AUTH\nStage"

  connect -> ehlo -> mail -> rcpt -> data -> auth
}

tables: "Database Tables" {
  style.fill: "#e3f2fd"
  blocked_ips_tbl: "blocked_ips\n• ip (PK)\n• reason\n• blocked_at\n• active"
  mta_conn_log_tbl: "mta_connection_log\n• id (PK)\n• client_ip\n• stage\n• action\n• timestamp"
  mta_msg_log_tbl: "mta_message_log\n• id (PK)\n• from_address\n• to_addresses\n• subject\n• message_size"
  categories_tbl: "message_categories\n• id (PK)\n• name\n• email_address\n• active"
  subscriptions_tbl: "subscriptions\n• id (PK)\n• subscriber_email\n• category_id (FK)\n• active"
}

logic: "Processing Logic" {
  style.fill: "#fffde7"
  ip_check: "IP Blocking\nCheck"
  email_validation: "Email Format\nValidation"
  category_check: "Category\nValidation"
  subscription_check: "Subscription\nCheck"
  final_decision: "Final\nDecision" {
    shape: diamond
  }
}

actions: "Possible Actions" {
  style.fill: "#fce4ec"
  accept: "ACCEPT" {
    style.stroke: green
  }
  reject: "REJECT" {
    style.stroke: red
  }
  quarantine: "QUARANTINE" {
    style.stroke: orange
  }
}

connect -> ip_check
ip_check -> blocked_ips_tbl: "lookup" { style.stroke: red }
ip_check -> mta_conn_log_tbl: "log result" { style.stroke: blue }

ehlo -> email_validation
email_validation -> mta_conn_log_tbl: "log result" { style.stroke: blue }

mail -> email_validation

rcpt -> category_check
category_check -> categories_tbl: "lookup" { style.stroke: green }
category_check -> mta_conn_log_tbl: "log result" { style.stroke: blue }

data -> subscription_check
subscription_check -> subscriptions_tbl: "lookup" { style.stroke: green }
subscription_check -> categories_tbl: "lookup" { style.stroke: green }
subscription_check -> final_decision

final_decision -> mta_msg_log_tbl: "log message" { style.stroke: blue }

auth -> mta_conn_log_tbl: "log auth" { style.stroke: blue }

subscriptions_tbl -> categories_tbl: "category_id → id" {
  style.stroke: purple
}

final_decision -> accept: "subscribed" { style.stroke: green }
final_decision -> quarantine: "not subscribed" { style.stroke: orange }
ip_check -> reject: "blocked IP" { style.stroke: red }
email_validation -> reject: "invalid format" { style.stroke: red }
category_check -> reject: "invalid category" { style.stroke: red }
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
- **Purpose**: Full message processing and subscription validation
- **Checks**:
  - Subscription exists in `subscriptions` table
  - Subscription is active
  - Sender is subscribed to target category
- **Logging**: `mta_message_log` (detailed message information)
- **Actions**: 
  - ACCEPT: Subscriber sending to subscribed category
  - QUARANTINE: Non-subscriber or inactive subscription
  - REJECT: System errors or policy violations

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

### Subscription Validation (DATA)
```rust
if subscription.active && subscription.category_id == target_category.id 
   && subscription.subscriber_email == sender_email {
    return ACCEPT;
} else {
    return QUARANTINE; // Could be legitimate but unsubscribed
}
```