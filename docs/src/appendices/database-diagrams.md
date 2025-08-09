# Database Diagrams

This appendix contains all database diagrams for the Kommunikationszentrum SpacetimeDB schema with detailed explanations.

## Complete Database Schema

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

The complete database schema diagram shows:

- **All tables** with their field names and data types
- **Primary keys** (PK) and auto-increment fields
- **Foreign key relationships** with solid blue arrows
- **Logical relationships** with dashed blue arrows
- **Functional grouping** of tables by color

### Table Groups by Color

- **Yellow (User Management)**: `account`
- **Green (Category Management)**: `message_categories`, `subscriptions`
- **Blue (MTA Processing)**: `mta_connection_log`, `mta_message_log`, `blocked_ips`

## Simplified Entity-Relationship Diagram

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

The simplified ER diagram focuses on the core relationships:

- **Clear entity boxes** with primary functional areas
- **Relationship lines** showing connections
- **Reduced complexity** for quick understanding
- **Color coding** by functional area

### Key Relationships

- `subscriptions.category_id` → `message_categories.id` (Foreign Key)
- `subscriptions.subscriber_account_id` → `account.id` (Logical relationship)
- MTA log tables are independent audit tables

## MTA Processing Flow

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

The MTA processing flow diagram illustrates:

### Processing Stages

1. **CONNECT** (Start): Initial connection validation
   - IP blocking check against `blocked_ips` table
   - Decision: ACCEPT or REJECT

2. **EHLO**: Extended HELO validation
   - Basic protocol validation
   - HELO string format check

3. **MAIL FROM**: Sender validation
   - Email address format validation
   - Future: Sender whitelist/blacklist

4. **RCPT TO**: Recipient validation
   - Category validation against `message_categories`
   - REJECT if category doesn't exist

5. **DATA**: Full message processing
   - Subscription validation against `subscriptions` table
   - Final ACCEPT/REJECT/QUARANTINE decision

6. **AUTH**: Authentication (currently accept-all)

### Decision Points

Each stage can result in:
- **ACCEPT** (Green): Continue to next stage
- **REJECT** (Red): Reject email immediately  
- **QUARANTINE** (Orange): Hold for manual review

### Database Interactions

- **Lookups**: Read from `blocked_ips`, `message_categories`, `subscriptions`
- **Logging**: Write to `mta_connection_log` and `mta_message_log`
- **Privacy**: IP addresses redacted in logs as "[REDACTED]"

## Using mdbook-graphviz

All diagrams in this documentation are rendered using mdbook-graphviz, which processes DOT code blocks directly. This means:

### Advantages
- **No image files needed**: Diagrams are generated at build time
- **Always up-to-date**: Diagrams can't become stale
- **Scalable**: SVG output scales perfectly
- **Version controlled**: DOT source is in git with the documentation

### Updating Diagrams
To update any diagram, simply edit the DOT code in the markdown file and rebuild the documentation:

```bash
cd docs
mdbook build
# or for live preview:
mdbook serve
```

### DOT Syntax Reference
The diagrams use Graphviz DOT syntax. Key elements:

- **Nodes**: `node_name [label="Display Text", shape=box, fillcolor=lightblue];`
- **Edges**: `node1 -> node2 [label="Relationship", color=red];`
- **Subgraphs**: `subgraph cluster_name { label="Group Name"; node1; node2; }`
- **Styling**: Colors, shapes, fonts, and layout options

For more complex diagrams, refer to the [Graphviz documentation](https://graphviz.org/doc/info/lang.html).

The diagrams provide visual documentation that complements the textual descriptions and help developers understand the system structure at a glance.

### Processing Stages

1. **CONNECT** (Start): Initial connection validation
   - IP blocking check against `blocked_ips` table
   - Decision: ACCEPT or REJECT

2. **EHLO**: Extended HELO validation
   - Basic protocol validation
   - HELO string format check

3. **MAIL FROM**: Sender validation
   - Email address format validation
   - Future: Sender whitelist/blacklist

4. **RCPT TO**: Recipient validation
   - Category validation against `message_categories`
   - REJECT if category doesn't exist

5. **DATA**: Full message processing
   - Subscription validation against `subscriptions` table
   - Final ACCEPT/REJECT/QUARANTINE decision

6. **AUTH**: Authentication (currently accept-all)

### Decision Points

Each stage can result in:
- **ACCEPT** (Green): Continue to next stage
- **REJECT** (Red): Reject email immediately  
- **QUARANTINE** (Orange): Hold for manual review

### Database Interactions

- **Lookups**: Read from `blocked_ips`, `message_categories`, `subscriptions`
- **Logging**: Write to `mta_connection_log` and `mta_message_log`
- **Privacy**: IP addresses redacted in logs as "[REDACTED]"

## Diagram Source Files

All diagrams are generated from DOT files using Graphviz:

### Database Schema
- **Source**: [`database_schema.dot`](../images/database_schema.dot)
- **Generate PNG**: `dot -Tpng database_schema.dot -o database_schema.png`
- **Generate SVG**: `dot -Tsvg database_schema.dot -o database_schema.svg`

### Simple ER Diagram  
- **Source**: [`simple_er_diagram.dot`](../images/simple_er_diagram.dot)
- **Generate PNG**: `dot -Tpng simple_er_diagram.dot -o simple_er_diagram.png`
- **Generate SVG**: `dot -Tsvg simple_er_diagram.dot -o simple_er_diagram.svg`

### MTA Processing Flow
- **Source**: [`mta_processing_flow.dot`](../images/mta_processing_flow.dot)  
- **Generate PNG**: `dot -Tpng mta_processing_flow.dot -o mta_processing_flow.png`
- **Generate SVG**: `dot -Tsvg mta_processing_flow.dot -o mta_processing_flow.svg`

## Regenerating Diagrams

To update the diagrams after schema changes:

### Prerequisites
```bash
# Install Graphviz on your system
sudo dnf install graphviz    # Fedora
sudo apt install graphviz   # Ubuntu/Debian
brew install graphviz       # macOS
```

### Generate All Formats
```bash
cd docs/src/images

# Database Schema
dot -Tpng database_schema.dot -o database_schema.png
dot -Tsvg database_schema.dot -o database_schema.svg

# Simple ER Diagram
dot -Tpng simple_er_diagram.dot -o simple_er_diagram.png
dot -Tsvg simple_er_diagram.dot -o simple_er_diagram.svg

# MTA Processing Flow
dot -Tpng mta_processing_flow.dot -o mta_processing_flow.png
dot -Tsvg mta_processing_flow.dot -o mta_processing_flow.svg
```

### Batch Script
```bash
#!/bin/bash
# regenerate-diagrams.sh
cd docs/src/images

for dot_file in *.dot; do
    base_name="${dot_file%.dot}"
    echo "Generating ${base_name}..."
    dot -Tpng "${dot_file}" -o "${base_name}.png"
    dot -Tsvg "${dot_file}" -o "${base_name}.svg"
done

echo "All diagrams regenerated!"
```

## Using Diagrams in Documentation

### Markdown Image Embedding
```markdown
![Alt Text](../images/diagram_name.png)
```

### Multiple Formats
```markdown
View as: [PNG](../images/diagram.png) | [SVG](../images/diagram.svg) | [Source](../images/diagram.dot)
```

### Responsive Images
For better responsive design, prefer SVG format when possible:
```markdown
![Database Schema](../images/database_schema.svg)
```

The diagrams provide visual documentation that complements the textual descriptions and help developers understand the system structure at a glance.
