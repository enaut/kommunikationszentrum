# Database Schema

The SpacetimeDB database schema for the Kommunikationszentrum consists of several interconnected tables that handle user management, email processing, and system logging.

## Schema Overview

The database is organized into three functional groups:

- **User Management**: `account`, `subscriptions`
- **Email Categories**: `message_categories`
- **MTA Processing**: `mta_connection_log`, `mta_message_log`, `blocked_ips`

## Visual Schema

```d2
direction: down

MTA Processing: {
  style.fill: lightblue

  MtaConnectionLog: {
    shape: sql_table
    id: "u64 (PK, auto_inc)"
    client_ip: "String"
    stage: "String"
    action: "String"
    timestamp: "i64"
    details: "String"
  }

  MtaMessageLog: {
    shape: sql_table
    id: "u64 (PK, auto_inc)"
    from_address: "String"
    to_addresses: "String (JSON)"
    subject: "String"
    message_size: "u64"
    stage: "String"
    action: "String"
    timestamp: "i64"
    queue_id: "Option<String>"
  }

  BlockedIp: {
    shape: sql_table
    ip: "String (PK)"
    reason: "String"
    blocked_at: "i64"
    active: "bool"
  }
}

Category Management: {
  style.fill: lightgreen

  MessageCategory: {
    shape: sql_table
    id: "u64 (PK, auto_inc)"
    name: "String"
    email_address: "String"
    description: "String"
    active: "bool"
  }

  Subscription: {
    shape: sql_table
    id: "u64 (PK, auto_inc)"
    subscriber_account_id: "u64"
    subscriber_email: "String"
    category_id: "u64 (FK)"
    subscribed_at: "i64"
    active: "bool"
  }
}

User Management: {
  style.fill: lightyellow

  Account: {
    shape: sql_table
    id: "u64 (PK)"
    identity: "Option<Identity>"
    name: "String"
    email: "String"
    is_active: "bool"
    last_synced: "i64"
  }
}


Category Management.Subscription -> Category Management.MessageCategory: {
  label: "category_id → id"
  style.stroke: blue
}

Category Management.Subscription -> User Management.Account: {
  label: "subscriber_account_id → id"
  style.stroke: blue
  style.stroke-dash: 4
}


MTA Processing.MtaConnectionLog -> MTA Processing.BlockedIp: {
  label: "checks IP blocking"
  style.stroke: red
  style.stroke-dash: 2
}

MTA Processing.MtaMessageLog -> Category Management.MessageCategory: {
  label: "validates recipients"
  style.stroke: green
  style.stroke-dash: 2
}

MTA Processing.MtaMessageLog -> Category Management.Subscription: {
  label: "checks subscriptions"
  style.stroke: green
  style.stroke-dash: 2
}
```

## Entity Relationships

```d2
direction: down

account: {
  label: "Account\n(User Management)"
  shape: rectangle
  style.fill: lightblue
}

mta_connection_log: {
  label: "MtaConnectionLog\n(MTA Processing)"
  shape: rectangle
  style.fill: lightcoral
}

mta_message_log: {
  label: "MtaMessageLog\n(MTA Processing)"
  shape: rectangle
  style.fill: lightcoral
}

blocked_ips: {
  label: "BlockedIp\n(MTA Security)"
  shape: rectangle
  style.fill: lightcoral
}

message_categories: {
  label: "MessageCategory\n(Category System)"
  shape: rectangle
  style.fill: lightgreen
}

subscriptions: {
  label: "Subscription\n(Category System)"
  shape: rectangle
  style.fill: lightgreen
}

subscriptions -> message_categories: {
  label: "belongs to"
  style.stroke: blue
  style.stroke-width: 3
}

subscriptions -> account: {
  label: "subscriber"
  style.stroke: blue
  style.stroke-width: 3
}

mta_connection_log -> blocked_ips: {
  label: "checks"
  style.stroke: red
  style.stroke-dash: 4
}

mta_message_log -> message_categories: {
  label: "validates"
  style.stroke: green
  style.stroke-dash: 4
}

mta_message_log -> subscriptions: {
  label: "verifies"
  style.stroke: green
  style.stroke-dash: 4
}
```

- `subscriptions` → `message_categories` (foreign key relationship)
- `account` table stores user data synchronized from Django
- MTA logs are independent audit tables

## Table Groups

### User Management Tables

**`account`**
- Stores user accounts synchronized from Django
- Links SpacetimeDB identity with user data
- Used for authentication and authorization

**`subscriptions`** 
- Links users to email categories they want to receive
- References `message_categories` via foreign key
- Supports active/inactive subscription states

### Email Category Management

**`message_categories`**
- Defines available email categories (mailing lists)
- Each category has an associated email address
- Used for routing decisions in MTA processing

### MTA Processing Tables

**`mta_connection_log`**
- Logs connection-level MTA events (CONNECT, EHLO, MAIL, RCPT, AUTH)
- Tracks IP addresses, stages, and actions
- Used for connection analysis and spam detection

**`mta_message_log`**
- Logs message-level MTA events (DATA stage)
- Stores message metadata (from, to, subject, size)
- Links to queue IDs for email tracking

**`blocked_ips`**
- IP blacklist for spam protection
- Supports active/inactive states
- Includes reason for blocking and timestamp

## Data Types

The schema uses standard SpacetimeDB data types:

- **Numeric**: `u64` for IDs, `i64` for timestamps
- **Text**: `String` for email addresses, names, and descriptions  
- **Boolean**: `bool` for active/inactive flags
- **JSON**: `String` fields storing JSON data (e.g., `to_addresses` array)
- **Optional**: `Option<T>` for nullable fields

## Primary Keys and Auto-Increment

- Most tables use `u64` auto-increment primary keys (`#[auto_inc]`)
- Exception: `blocked_ips` uses IP address as natural primary key
- Auto-increment ensures unique IDs across database lifecycle

## Foreign Key Relationships

The schema has one explicit foreign key relationship:

```rust
// subscriptions table references message_categories
category_id: u64  // → message_categories.id
```

Other relationships are maintained through application logic rather than database constraints.

## Privacy Considerations

The schema is designed with privacy in mind:

- IP addresses in logs can be redacted as "[REDACTED]"
- Email content is not stored, only metadata
- Personal data is minimized to essential fields only

For detailed field definitions, see [Tables Reference](./tables-reference.md).
