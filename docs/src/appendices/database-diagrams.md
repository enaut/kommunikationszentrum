# Database Diagrams

This appendix contains all database diagrams for the Kommunikationszentrum SpacetimeDB schema with detailed explanations.

## Complete Database Schema

```d2
direction: down

account: {
  label: "Account\nid: u64 (PK)\nidentity: Option<Identity>\nname: String\nemail: String\nis_active: bool\nlast_synced: i64"
}

mta_connection_log: {
  label: "MtaConnectionLog\nid: u64 (PK, auto_inc)\nclient_ip: String\nstage: String\naction: String\ntimestamp: i64\ndetails: String"
}

mta_message_log: {
  label: "MtaMessageLog\nid: u64 (PK, auto_inc)\nfrom_address: String\nto_addresses: String (JSON)\nsubject: String\nmessage_size: u64\nstage: String\naction: String\ntimestamp: i64\nqueue_id: Option<String>"
}

blocked_ips: {
  label: "BlockedIp\nip: String (PK)\nreason: String\nblocked_at: i64\nactive: bool"
}

message_categories: {
  label: "MessageCategory\nid: u64 (PK, auto_inc)\nname: String\nemail_address: String\ndescription: String\nactive: bool"
}

subscriptions: {
  label: "Subscription\nid: u64 (PK, auto_inc)\nsubscriber_account_id: u64\nsubscriber_email: String\ncategory_id: u64 (FK)\nsubscribed_at: i64\nactive: bool"
}

subscriptions -> message_categories: "category_id → id" {
  style.stroke: blue
}
subscriptions -> account: "subscriber_account_id → id" {
  style.stroke: blue
  style.stroke-dash: 5
}

mta_connection_log -> blocked_ips: "checks IP blocking" {
  style.stroke: red
  style.stroke-dash: 2
}
mta_message_log -> message_categories: "validates recipients" {
  style.stroke: green
  style.stroke-dash: 2
}
mta_message_log -> subscriptions: "checks subscriptions" {
  style.stroke: green
  style.stroke-dash: 2
}

mta_processing: "MTA Processing" {
  style.fill: "#e3f2fd"
  mta_connection_log
  mta_message_log
  blocked_ips
}

category_mgmt: "Category Management" {
  style.fill: "#e8f5e9"
  message_categories
  subscriptions
}

user_mgmt: "User Management" {
  style.fill: "#fffde7"
  account
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

```d2
direction: down

account: "Account\n(User Management)" {
  style.fill: lightblue
}
mta_connection_log: "MtaConnectionLog\n(MTA Processing)" {
  style.fill: lightcoral
}
mta_message_log: "MtaMessageLog\n(MTA Processing)" {
  style.fill: lightcoral
}
blocked_ips: "BlockedIp\n(MTA Security)" {
  style.fill: lightcoral
}
message_categories: "MessageCategory\n(Category System)" {
  style.fill: lightgreen
}
subscriptions: "Subscription\n(Category System)" {
  style.fill: lightgreen
}

subscriptions -> message_categories: "belongs to" {
  style.stroke: blue
}
subscriptions -> account: "subscriber" {
  style.stroke: blue
}

mta_connection_log -> blocked_ips: "checks" {
  style.stroke: red
  style.stroke-dash: 5
}
mta_message_log -> message_categories: "validates" {
  style.stroke: green
  style.stroke-dash: 5
}
mta_message_log -> subscriptions: "verifies" {
  style.stroke: green
  style.stroke-dash: 5
}
```

## MTA Processing Flow

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

## Using mdbook-d2

All diagrams in this documentation are rendered using mdbook-d2, which compiles D2 syntax directly during mdBook building. This means:

### Advantages
- **No image files needed**: Diagrams are generated at build time
- **Clean and readable syntax**: Native layout and nesting support
- **Vector output**: SVGs that scale perfectly and support dark mode
- **Plain-text diagram definitions**: Fully diff-friendly and version-controlled with documentation

### Updating Diagrams
To update a diagram, simply edit the D2 code in the markdown file and rebuild:

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
