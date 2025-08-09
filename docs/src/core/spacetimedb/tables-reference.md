# Tables Reference

This chapter provides detailed field definitions for all SpacetimeDB tables in the Kommunikationszentrum.

## User Management Tables

### `account`

Stores user accounts synchronized from Django solawispielplatz.

```rust
#[spacetimedb::table(name = account, public)]
pub struct Account {
    #[primary_key]
    pub id: u64,                    // Django user ID (mitgliedsnr)
    pub identity: Option<Identity>, // SpacetimeDB identity when connected
    pub name: String,               // User's display name
    pub email: String,              // Primary email address
    pub is_active: bool,            // Account active status
    pub last_synced: i64,           // Timestamp of last Django sync
}
```

**Field Details:**
- `id`: Primary key, corresponds to Django user's membership number
- `identity`: Set when user connects via authenticated WebSocket
- `name`: User's full name from Django user profile
- `email`: Primary email address for the user
- `is_active`: Whether the account is currently active
- `last_synced`: Unix timestamp of last synchronization from Django

### `subscriptions`

Links users to email categories they want to receive.

```rust
#[spacetimedb::table(name = subscriptions)]
pub struct Subscription {
    #[primary_key]
    #[auto_inc]
    pub id: u64,                    // Auto-increment primary key
    pub subscriber_account_id: u64, // References account.id
    pub subscriber_email: String,   // Email address of subscriber
    pub category_id: u64,           // Foreign key to message_categories.id
    pub subscribed_at: i64,         // Timestamp when subscription was created
    pub active: bool,               // Whether subscription is currently active
}
```

**Field Details:**
- `id`: Auto-increment primary key
- `subscriber_account_id`: Links to `account.id` 
- `subscriber_email`: Email address that will receive category emails
- `category_id`: **Foreign key** to `message_categories.id`
- `subscribed_at`: Unix timestamp when subscription was created
- `active`: Whether the subscription is currently active

## Email Category Management

### `message_categories`

Defines available email categories (mailing lists).

```rust
#[spacetimedb::table(name = message_categories)]
pub struct MessageCategory {
    #[primary_key]
    #[auto_inc]
    pub id: u64,            // Auto-increment primary key
    pub name: String,       // Human-readable category name
    pub email_address: String, // Email address for this category
    pub description: String,   // Description of the category
    pub active: bool,          // Whether category is currently active
}
```

**Field Details:**
- `id`: Auto-increment primary key, referenced by subscriptions
- `name`: Display name (e.g., "SoLaWi News", "Events", "General")
- `email_address`: Category email (e.g., "news@solawi.org", "events@solawi.org")
- `description`: Longer description explaining the category's purpose
- `active`: Whether the category is currently accepting subscriptions

## MTA Processing Tables

### `mta_connection_log`

Logs connection-level MTA events from Stalwart MTA hooks.

```rust
#[spacetimedb::table(name = mta_connection_log)]
pub struct MtaConnectionLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,            // Auto-increment primary key
    pub client_ip: String,  // Client IP address (may be redacted)
    pub stage: String,      // MTA stage (CONNECT, EHLO, MAIL, RCPT, AUTH)
    pub action: String,     // Action taken (ACCEPT, REJECT, QUARANTINE)
    pub timestamp: i64,     // Unix timestamp of the event
    pub details: String,    // Additional details about the event
}
```

**Field Details:**
- `id`: Auto-increment primary key
- `client_ip`: IP address of connecting client (may be "[REDACTED]" for privacy)
- `stage`: MTA processing stage where event occurred
- `action`: Decision made by the system
- `timestamp`: Unix timestamp when event occurred
- `details`: Additional context or error information

**MTA Stages:**
- `CONNECT`: Initial connection to MTA
- `EHLO`: Extended HELO command
- `MAIL`: MAIL FROM command  
- `RCPT`: RCPT TO command
- `AUTH`: Authentication attempt

**Actions:**
- `ACCEPT`: Allow the operation to continue
- `REJECT`: Reject the operation
- `QUARANTINE`: Hold for manual review

### `mta_message_log`

Logs message-level MTA events (DATA stage processing).

```rust
#[spacetimedb::table(name = mta_message_log)]
pub struct MtaMessageLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,                // Auto-increment primary key
    pub from_address: String,   // Sender email address
    pub to_addresses: String,   // JSON array of recipient addresses
    pub subject: String,        // Email subject line
    pub message_size: u64,      // Size of message in bytes
    pub stage: String,          // MTA stage (typically "DATA")
    pub action: String,         // Action taken (ACCEPT, REJECT, QUARANTINE)
    pub timestamp: i64,         // Unix timestamp of processing
    pub queue_id: Option<String>, // MTA queue ID for tracking
}
```

**Field Details:**
- `id`: Auto-increment primary key
- `from_address`: Email address of the sender
- `to_addresses`: JSON string containing array of recipient emails
- `subject`: Subject line of the email (may be truncated)
- `message_size`: Size of the message in bytes
- `stage`: Usually "DATA" for message-level processing
- `action`: Decision made by the subscription system
- `timestamp`: Unix timestamp when message was processed
- `queue_id`: Optional MTA queue ID for email tracking

### `blocked_ips`

IP blacklist for spam protection.

```rust
#[spacetimedb::table(name = blocked_ips)]
pub struct BlockedIp {
    #[primary_key]
    pub ip: String,         // IP address (primary key)
    pub reason: String,     // Reason for blocking
    pub blocked_at: i64,    // Timestamp when IP was blocked
    pub active: bool,       // Whether block is currently active
}
```

**Field Details:**
- `ip`: **Primary key** - IP address in string format
- `reason`: Human-readable reason for blocking (e.g., "Spam source", "Brute force")
- `blocked_at`: Unix timestamp when IP was added to blocklist
- `active`: Whether the block is currently enforced

## Data Type Reference

### SpacetimeDB Types Used

- `u64`: 64-bit unsigned integer, used for IDs and sizes
- `i64`: 64-bit signed integer, used for Unix timestamps  
- `String`: UTF-8 text strings
- `bool`: Boolean true/false values
- `Option<T>`: Nullable fields (Some(value) or None)
- `Identity`: SpacetimeDB client identity type

### JSON Fields

Some `String` fields contain JSON data:

**`mta_message_log.to_addresses`** example:
```json
["user1@example.org", "user2@example.org", "category@solawi.org"]
```

### Timestamp Format

All timestamps use Unix time (seconds since January 1, 1970 UTC):

```rust
let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs() as i64;
```

## Table Relationships

### Foreign Key Constraints

The schema has one explicit foreign key relationship:

```
subscriptions.category_id → message_categories.id
```

### Logical Relationships

Other relationships are maintained by application logic:

- `subscriptions.subscriber_account_id` → `account.id`
- MTA logs reference external systems via string identifiers
