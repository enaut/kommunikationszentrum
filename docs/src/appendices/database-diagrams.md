# Database Diagrams

This appendix contains all database diagrams for the Kommunikationszentrum SpacetimeDB schema with detailed explanations.

## Complete Database Schema

```d2
{{#include database-schema-complete.d2}}
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
{{#include database-schema-er-simplified.d2}}
```

## MTA Processing Flow

```d2
{{#include mta-processing-flow.d2}}
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
