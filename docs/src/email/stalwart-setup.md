# Stalwart MTA Setup

The Kommunikationszentrum integrates with the Stalwart MTA (Mail Transfer Agent) to process incoming emails through a hook-based system. This document covers the Stalwart-side configuration required to POST hooks directly to the SpacetimeDB module HTTP routes.

## Overview

Stalwart MTA supports webhook-based processing hooks. The integration enables the Kommunikationszentrum to:

- Validate incoming emails against subscription lists
- Block spam and unwanted content
- Route emails based on categories
- Log all email processing activity

## Architecture

```dot process
digraph stalwart_integration {
    rankdir=LR;
    node [shape=box, fontname="Arial", fontsize=10];
    edge [fontname="Arial", fontsize=8];
    
    // External components
    internet [label="Internet\n(Email Senders)", shape=ellipse, fillcolor=lightcyan, style=filled];
    
    // Stalwart MTA stages
    subgraph cluster_stalwart {
        label="Stalwart MTA Processing";
        style=filled;
        fillcolor=lightblue;
        
        smtp_server [label="SMTP Server"];
        connect_stage [label="CONNECT Stage"];
        ehlo_stage [label="EHLO Stage"];
        mail_stage [label="MAIL FROM Stage"];
        rcpt_stage [label="RCPT TO Stage"];
        data_stage [label="DATA Stage"];
        auth_stage [label="AUTH Stage"];
        
        smtp_server -> connect_stage -> ehlo_stage -> mail_stage -> rcpt_stage -> data_stage -> auth_stage;
    }
    
    // Kommunikationszentrum components
    subgraph cluster_komm {
        label="Kommunikationszentrum";
        style=filled;
        fillcolor=lightgreen;
        
        spacetimedb [label="SpacetimeDB\nPort 3000\nModule HTTP Routes", shape=box];
        
        spacetimedb;
    }
    
    // Email delivery
    local_delivery [label="Local Delivery\n(Accepted Emails)", shape=ellipse, fillcolor=lightgreen, style=filled];
    rejection [label="Rejection\n(Blocked Emails)", shape=ellipse, fillcolor=lightcoral, style=filled];
    
    // Flow connections
    internet -> smtp_server;
    
    // Hook connections (each stage posts to the module route)
    connect_stage -> spacetimedb [label="HTTP Hook", style=dashed, color=red];
    ehlo_stage -> spacetimedb [label="HTTP Hook", style=dashed, color=red];
    mail_stage -> spacetimedb [label="HTTP Hook", style=dashed, color=red];
    rcpt_stage -> spacetimedb [label="HTTP Hook", style=dashed, color=red];
    data_stage -> spacetimedb [label="HTTP Hook", style=dashed, color=red];
    auth_stage -> spacetimedb [label="HTTP Hook", style=dashed, color=red];
    
    // Final decisions
    auth_stage -> local_delivery [label="ACCEPT"];
    auth_stage -> rejection [label="REJECT"];
}
```

## Prerequisites

Before configuring Stalwart hooks, ensure you have:

1. **Stalwart MTA installed** and running
2. **Kommunikationszentrum module published**:
   - SpacetimeDB server running and the `kommunikation` module published (port 3000)
3. **Network connectivity** between Stalwart and the SpacetimeDB host
4. **Administrative access** to Stalwart configuration

## Basic Stalwart Configuration

Hook Configuration:

Add the following to your Stalwart MTA configuration file (typically `/etc/stalwart-mail/config.toml`) and point it at the module route:

```toml
[session.hook]
# URL of the module HTTP route for MTA hooks
url = "http://localhost:3000/v1/database/kommunikation/route/mta-hook"

# Timeout for webhook responses
timeout = "30s"

# Retry configuration
retry.max = 3
retry.delay = "1s"
```

You must also ensure Stalwart sends an `Authorization: Bearer <token>` header. Configure Stalwart's hook client appropriately or place a proxy in front of SpacetimeDB that injects the header.

Hook Stages:

Configure which stages should trigger hooks:

```toml
[session.hook.stage]
# Connection validation
connect = true

# EHLO/HELO validation  
ehlo = true

# Sender validation
mail = true

# Recipient validation  
rcpt = true

# Message content processing
data = true

# Authentication handling
auth = true
```

Error Handling:

Configure how Stalwart handles webhook errors (application-specific policy):

```toml
[session.hook.error]
# Action when webhook is unavailable
# Options: accept, reject, quarantine
on_unavailable = "quarantine"

# Action when webhook times out
on_timeout = "quarantine"

# Action when webhook returns invalid response
on_invalid = "reject"
```

## Testing the Configuration

### 1. Configuration Validation

Test your Stalwart configuration:

```bash
stalwart-mail --config /etc/stalwart-mail/config.toml --dry-run
```

### 2. Hook Connectivity Test

Verify that Stalwart can reach the module route (use the token the module expects):

```bash
curl -X POST "http://localhost:3000/v1/database/kommunikation/route/mta-hook" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "context": {
      "stage": "connect",
      "client": {
        "ip": "127.0.0.1",
        "helo": "test.example.com"
      }
    }
  }'
```

Expected response:
```json
{
  "action": "accept"
}
```

# Next Steps

After setting up Stalwart MTA:

1. Configure [MTA Hook Configuration](./mta-hook-config.md) for detailed hook handling
2. Set up [Email Categories](./categories.md) for content routing  
3. Implement [Subscription System](./subscriptions.md) for user management
4. Review [Processing Flow](./processing-flow.md) for understanding the decision logic
