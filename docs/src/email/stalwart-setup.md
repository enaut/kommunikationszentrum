# Stalwart MTA Setup

The Kommunikationszentrum integrates with the Stalwart MTA (Mail Transfer Agent) to process incoming emails through a hook-based system. This document covers the Stalwart-side configuration required to POST hooks directly to the SpacetimeDB module HTTP routes.

## Overview

Stalwart MTA supports webhook-based processing hooks. The integration enables the Kommunikationszentrum to:

- Validate incoming emails against subscription lists
- Block spam and unwanted content
- Route emails based on categories
- Log all email processing activity

## Architecture

```d2
direction: right

internet: "Internet\n(Email Senders)" {
  shape: oval
  style.fill: lightcyan
}

stalwart: "Stalwart MTA Processing" {
  style.fill: "#e3f2fd"
  smtp_server: "SMTP Server"
  connect_stage: "CONNECT Stage"
  ehlo_stage: "EHLO Stage"
  mail_stage: "MAIL FROM Stage"
  rcpt_stage: "RCPT TO Stage"
  data_stage: "DATA Stage"
  auth_stage: "AUTH Stage"

  smtp_server -> connect_stage -> ehlo_stage -> mail_stage -> rcpt_stage -> data_stage -> auth_stage
}

spacetimedb: "SpacetimeDB\nPort 3000\nModule HTTP Routes" {
  style.fill: lightgreen
}

local_delivery: "Local Delivery\n(Accepted Emails)" {
  shape: oval
  style.fill: lightgreen
}
rejection: "Rejection\n(Blocked Emails)" {
  shape: oval
  style.fill: lightcoral
}

internet -> smtp_server

connect_stage -> spacetimedb: "HTTP Hook" { style.stroke: red; style.stroke-dash: 5 }
ehlo_stage -> spacetimedb: "HTTP Hook" { style.stroke: red; style.stroke-dash: 5 }
mail_stage -> spacetimedb: "HTTP Hook" { style.stroke: red; style.stroke-dash: 5 }
rcpt_stage -> spacetimedb: "HTTP Hook" { style.stroke: red; style.stroke-dash: 5 }
data_stage -> spacetimedb: "HTTP Hook" { style.stroke: red; style.stroke-dash: 5 }
auth_stage -> spacetimedb: "HTTP Hook" { style.stroke: red; style.stroke-dash: 5 }

auth_stage -> local_delivery: "ACCEPT"
auth_stage -> rejection: "REJECT"
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
