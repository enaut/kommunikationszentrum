# Processing Flow

The email processing flow in the Kommunikationszentrum follows a multi-stage validation process based on MTA hooks from Stalwart.

## MTA Processing Pipeline

```dot process
digraph mta_processing_flow {
    // Graph settings
    rankdir=LR;
    node [shape=box, fontname="Arial", fontsize=10];
    edge [fontname="Arial", fontsize=8];
    
    // MTA Hook Stages
    subgraph cluster_stages {
        label="MTA Hook Stages";
        style=filled;
        fillcolor=lightcyan;
        
        connect [label="CONNECT\nStage"];
        ehlo [label="EHLO\nStage"];
        mail [label="MAIL FROM\nStage"];
        rcpt [label="RCPT TO\nStage"];
        data [label="DATA\nStage"];
        auth [label="AUTH\nStage"];
        
        connect -> ehlo -> mail -> rcpt -> data -> auth;
    }
    
    // Database Tables
    subgraph cluster_tables {
        label="Database Tables";
        style=filled;
        fillcolor=lightblue;
        
        blocked_ips_tbl [label="blocked_ips\n• ip (PK)\n• reason\n• blocked_at\n• active", shape=record];
        mta_conn_log_tbl [label="mta_connection_log\n• id (PK)\n• client_ip\n• stage\n• action\n• timestamp\n• details", shape=record];
        mta_msg_log_tbl [label="mta_message_log\n• id (PK)\n• from_address\n• to_addresses\n• subject\n• message_size\n• stage\n• action\n• timestamp\n• queue_id", shape=record];
        categories_tbl [label="message_categories\n• id (PK)\n• name\n• email_address\n• description\n• active", shape=record];
        subscriptions_tbl [label="subscriptions\n• id (PK)\n• subscriber_email\n• category_id (FK)\n• subscribed_at\n• active", shape=record];
    }
    
    // Processing Logic
    subgraph cluster_logic {
        label="Processing Logic";
        style=filled;
        fillcolor=lightyellow;
        
        ip_check [label="IP Blocking\nCheck"];
        email_validation [label="Email Format\nValidation"];
        category_check [label="Category\nValidation"];
        subscription_check [label="Subscription\nCheck"];
        final_decision [label="Final\nDecision", shape=diamond];
    }
    
    // Flow connections
    connect -> ip_check;
    ip_check -> blocked_ips_tbl [label="lookup", color="red"];
    ip_check -> mta_conn_log_tbl [label="log result", color="blue"];
    
    ehlo -> email_validation;
    email_validation -> mta_conn_log_tbl [label="log result", color="blue"];
    
    mail -> email_validation;
    
    rcpt -> category_check;
    category_check -> categories_tbl [label="lookup", color="green"];
    category_check -> mta_conn_log_tbl [label="log result", color="blue"];
    
    data -> subscription_check;
    subscription_check -> subscriptions_tbl [label="lookup", color="green"];
    subscription_check -> categories_tbl [label="lookup", color="green"];
    subscription_check -> final_decision;
    final_decision -> mta_msg_log_tbl [label="log message", color="blue"];
    
    auth -> mta_conn_log_tbl [label="log auth", color="blue"];
    
    // Foreign key relationship
    subscriptions_tbl -> categories_tbl [label="category_id → id", color="purple", style=bold];
    
    // Actions
    subgraph cluster_actions {
        label="Possible Actions";
        style=filled;
        fillcolor=lightpink;
        
        accept [label="ACCEPT", shape=ellipse, color="green"];
        reject [label="REJECT", shape=ellipse, color="red"];
        quarantine [label="QUARANTINE", shape=ellipse, color="orange"];
    }
    
    final_decision -> accept [label="subscribed", color="green"];
    final_decision -> quarantine [label="not subscribed", color="orange"];
    ip_check -> reject [label="blocked IP", color="red"];
    email_validation -> reject [label="invalid format", color="red"];
    category_check -> reject [label="invalid category", color="red"];
}
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