# MTA Hook Configuration

The MTA Hook Configuration defines how the Kommunikationszentrum webhook proxy processes hooks from Stalwart MTA. This document covers the configuration options, hook formats, and processing logic.

## Hook Protocol

The Kommunikationszentrum uses the standard Stalwart MTA hook protocol, which sends HTTP POST requests with JSON payloads for each SMTP stage.

### Hook Request Format

```json
{
  "context": {
    "stage": "connect|ehlo|mail|rcpt|data|auth",
    "client": {
      "ip": "192.168.1.100",
      "helo": "sender.example.com",
      "hostname": "mail.example.com"
    },
    "session": {
      "id": "session-uuid",
      "timestamp": 1672531200
    }
  },
  "envelope": {
    "from": {
      "address": "sender@example.com"
    },
    "to": [
      {
        "address": "recipient@solawi.org"
      }
    ]
  },
  "message": {
    "size": 1024,
    "headers": [
      ["Subject", "Test Message"],
      ["From", "sender@example.com"],
      ["To", "recipient@solawi.org"]
    ]
  }
}
```

### Hook Response Format

```json
{
  "action": "accept|reject|quarantine",
  "code": 250,
  "reason": "Message accepted",
  "modifications": [
    {
      "type": "add_header",
      "name": "X-Processed-By",
      "value": "Kommunikationszentrum"
    }
  ]
}
```

## Configuration File

The webhook proxy configuration is handled through environment variables and compile-time settings. Key configuration options:

### Environment Variables

```bash
# SpacetimeDB connection
SPACETIMEDB_URI=http://localhost:3000
SPACETIMEDB_MODULE=kommunikation

# Webhook proxy server
WEBHOOK_BIND_ADDRESS=0.0.0.0:3002
WEBHOOK_TIMEOUT=30

# Logging configuration
RUST_LOG=webhook_proxy=debug,mta_hook=info
LOG_FORMAT=json|text
LOG_REDACT_IPS=true
```

## Stage-Specific Configuration
- **CONNECT Stage**:

    - Current Implementation:
        - Accepts all connections (no IP blocking implemented yet)
        - Logs connection attempts
        - Uses hardcoded URI: `http://localhost:3000` for SpacetimeDB

    - Processing Logic:
        1. Check IP against `blocked_ips` table (placeholder - currently accepts all)
        2. Log connection attempt with timestamp

- **EHLO Stage**:

    - Current Implementation:
        - Performs basic validation: checks that the HELO/EHLO parameter is not empty.
        - Rejects with a 501 error code if the parameter is empty.

    - Processing Logic:
        1. Validate that the HELO/EHLO parameter is not empty.
        2. Log the result of the validation.
        3. Return ACCEPT if valid, REJECT (501) if invalid.

- **MAIL FROM Stage**:

    - Current Implementation:
        - Performs basic email validation: checks for the presence of the '@' character and that the address is not empty.
        - Rejects with a 550 error code if the format is invalid.

    - Processing Logic:
        1. Validate that the sender email address contains an '@' and is not empty.
        2. Log the sender information.
        3. Return ACCEPT for valid addresses, REJECT (550) for invalid ones.

- **RCPT TO Stage**:

    - Current Implementation:
        - Performs basic validation of the recipient email format.
        - Accepts all valid email formats; category validation is currently a placeholder.

    - Processing Logic:
        1. Validate that the recipient email address contains an '@' and is not empty.
        2. Log the result of the recipient validation.
        3. Return ACCEPT for valid addresses, REJECT (550) for invalid ones.

- **DATA Stage**:

    - Current Implementation:
        - Extracts message metadata such as size, subject, and headers.
        - Adds processing headers (`X-Processed-By`, `X-Processing-Time`).
        - Always accepts messages.

    - Processing Logic:
        1. Extract message metadata (size, subject, headers).
        2. Add processing headers to the message.
        3. Log complete message information.
        4. Return ACCEPT with modifications.

- **AUTH Stage**:

    - Current Implementation:
        - Pass-through: always accepts authentication attempts.
        - No authentication validation is performed.

    - Processing Logic:
        1. Accept all authentication attempts (placeholder).


## Database Integration

### Connection Configuration

**Current Hardcoded Values:**
- SpacetimeDB URI: `"http://localhost:3000"`
- Module Name: `"kommunikation"`
- Default bind address: `"0.0.0.0:3002"`

### Reducer Calls

The webhook proxy calls specific SpacetimeDB reducers for each operation:

```rust
// Core reducer call
self.db_connection.reducers.handle_mta_hook(hook_data)

// Logging reducers (called automatically by handle_mta_hook)
// - log_mta_connection()
// - log_mta_message() 
// - check_blocked_ip()
// - validate_category()
// - check_subscription()
```

## Error Handling

### Error Categories

```rust
pub enum MtaHookError {
    // Connection errors
    DatabaseConnection(String),
    DatabaseTimeout(String),
    
    // Validation errors  
    InvalidEmailFormat(String),
    UnknownCategory(String),
    NoSubscription(String),
    
    // System errors
    SerializationError(String),
    ConfigurationError(String),
}
```

### Error Responses

```rust
// Default error handling
match error {
    MtaHookError::DatabaseConnection(_) => MtaHookResponse::quarantine(),
    MtaHookError::InvalidEmailFormat(_) => MtaHookResponse::reject(550, "Invalid format"),
    MtaHookError::UnknownCategory(_) => MtaHookResponse::reject(550, "Unknown recipient"),
    MtaHookError::NoSubscription(_) => MtaHookResponse::quarantine(),
    _ => MtaHookResponse::reject(550, "Processing error"),
}
```

## Testing Configuration

Test Hook Generation:

Use the provided test script to generate sample hooks:

```bash
cd server/docs
./test-mta-hooks.sh
```

Integration Testing:

Test specific hook scenarios:

```bash
# Test CONNECT stage with blocked IP
curl -X POST http://localhost:3002/mta-hook \
  -H "Content-Type: application/json" \
  -d @test_data/blocked_ip_hook.json

# Test RCPT stage with unknown category  
curl -X POST http://localhost:3002/mta-hook \
  -H "Content-Type: application/json" \
  -d @test_data/unknown_category_hook.json
  
# Test DATA stage with subscription check
curl -X POST http://localhost:3002/mta-hook \
  -H "Content-Type: application/json" \
  -d @test_data/subscription_test_hook.json
```

### Available Endpoints

- `/mta-hook` - Main MTA hook processing endpoint  
- `/user-sync` - User synchronization endpoint