# Database Schema

The SpacetimeDB database schema for the Kommunikationszentrum consists of several interconnected tables that handle user management, email processing, and system logging.

## Schema Overview

The database is organized into three functional groups:

- **User Management**: `account`, `subscriptions`
- **Email Categories**: `message_categories`
- **MTA Processing**: `mta_connection_log`, `mta_message_log`, `blocked_ips`

## Visual Schema

```dot process
digraph kommunikationszentrum_db {
    // Graph settings
    rankdir=TB;
    node [shape=record, fontname="Arial", fontsize=10];
    edge [fontname="Arial", fontsize=8];
    
    // Updated table definitions based on actual schema
    account [label="{Account|id: u64 (PK)\lidentity: Option\<Identity\>\lname: String\lemail: String\lis_active: bool\llast_synced: i64\l}"];
    
    mta_connection_log [label="{MtaConnectionLog|id: u64 (PK, auto_inc)\lclient_ip: String\lstage: String\laction: String\ltimestamp: i64\ldetails: String\l}"];
    
    mta_message_log [label="{MtaMessageLog|id: u64 (PK, auto_inc)\lfrom_address: String\lto_addresses: String (JSON)\lsubject: String\lmessage_size: u64\lstage: String\laction: String\ltimestamp: i64\lqueue_id: Option\<String\>\l}"];
    
    blocked_ips [label="{BlockedIp|ip: String (PK)\lreason: String\lblocked_at: i64\lactive: bool\l}"];
    
    message_categories [label="{MessageCategory|id: u64 (PK, auto_inc)\lname: String\lemail_address: String\ldescription: String\lactive: bool\l}"];
    
    subscriptions [label="{Subscription|id: u64 (PK, auto_inc)\lsubscriber_account_id: u64\lsubscriber_email: String\lcategory_id: u64 (FK)\lsubscribed_at: i64\lactive: bool\l}"];
    
    // Relationships
    subscriptions -> message_categories [label="category_id → id", color="blue"];
    subscriptions -> account [label="subscriber_account_id → id", color="blue", style=dashed];
    
    // Data flow relationships (dotted lines)
    mta_connection_log -> blocked_ips [style=dotted, label="checks IP blocking", color="red"];
    mta_message_log -> message_categories [style=dotted, label="validates recipients", color="green"];
    mta_message_log -> subscriptions [style=dotted, label="checks subscriptions", color="green"];
    
    // Grouping by functionality
    subgraph cluster_mta {
        label="MTA Processing";
        style=filled;
        fillcolor=lightblue;
        mta_connection_log;
        mta_message_log;
        blocked_ips;
    }
    
    subgraph cluster_categories {
        label="Category Management";
        style=filled;
        fillcolor=lightgreen;
        message_categories;
        subscriptions;
    }
    
    subgraph cluster_users {
        label="User Management";
        style=filled;
        fillcolor=lightyellow;
        account;
    }
}
```

## Entity Relationships

```dot process
digraph simple_er_diagram {
    // Graph settings
    rankdir=TB;
    node [shape=box, fontname="Arial", fontsize=12, style=filled];
    edge [fontname="Arial", fontsize=10];
    
    // Entity colors
    account [fillcolor=lightblue, label="Account\n(User Management)"];
    
    mta_connection_log [fillcolor=lightcoral, label="MtaConnectionLog\n(MTA Processing)"];
    mta_message_log [fillcolor=lightcoral, label="MtaMessageLog\n(MTA Processing)"];
    blocked_ips [fillcolor=lightcoral, label="BlockedIp\n(MTA Security)"];
    
    message_categories [fillcolor=lightgreen, label="MessageCategory\n(Category System)"];
    subscriptions [fillcolor=lightgreen, label="Subscription\n(Category System)"];
    
    // Primary relationships
    subscriptions -> message_categories [label="belongs to", style=bold, color=blue];
    subscriptions -> account [label="subscriber", style=bold, color=blue];
    
    // Functional relationships (dotted)
    mta_connection_log -> blocked_ips [label="checks", style=dashed, color=red];
    mta_message_log -> message_categories [label="validates", style=dashed, color=green];
    mta_message_log -> subscriptions [label="verifies", style=dashed, color=green];
    
    // Grouping
    {rank=same; account;}
    {rank=same; mta_connection_log; mta_message_log; blocked_ips;}
    {rank=same; message_categories; subscriptions;}
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
