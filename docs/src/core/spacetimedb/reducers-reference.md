# Reducers Reference

Reducers are the business logic functions in SpacetimeDB that handle all database operations and system events. This chapter documents all reducers available in the Kommunikationszentrum.

## System Lifecycle Reducers

### `init`
```rust
#[spacetimedb::reducer(init)]
pub fn init(_ctx: &ReducerContext)
```
Called when the SpacetimeDB module is initially published. Currently performs no operations but can be extended for initial setup tasks.

### `identity_connected`
```rust
#[spacetimedb::reducer(client_connected)]
pub fn identity_connected(ctx: &ReducerContext)
```
Called every time a new client connects via WebSocket. Logs the connection and can be extended to:
- Check if the identity is authorized
- Link the identity to an account in the database
- Set up user-specific permissions

### `identity_disconnected`
```rust
#[spacetimedb::reducer(client_disconnected)]
pub fn identity_disconnected(_ctx: &ReducerContext)
```
Called every time a client disconnects. Currently performs no operations.

## MTA Hook Processing

### `handle_mta_hook`
```rust
#[spacetimedb::reducer]
pub fn handle_mta_hook(ctx: &ReducerContext, hook_data: String)
```

**Purpose**: Main entry point for processing MTA hooks from Stalwart email server.

**Parameters**:
- `hook_data`: JSON string containing the MTA hook request

**Behavior**:
- Parses the incoming JSON into `MtaHookRequest`
- Routes to appropriate stage handler based on `request.context.stage`
- Logs all operations with redacted sensitive information

**Stage Handlers**:
- `handle_connect_stage`: IP blocking validation
- `handle_ehlo_stage`: HELO/EHLO validation
- `handle_mail_stage`: MAIL FROM validation
- `handle_rcpt_stage`: RCPT TO and category validation
- `handle_data_stage`: Full message processing with subscription checks
- `handle_auth_stage`: Authentication handling (currently accept-all)

**Processing Flow**:
1. Parse JSON hook data
2. Extract timestamp from context
3. Route to stage-specific handler
4. Log connection and/or message details
5. Make ACCEPT/REJECT/QUARANTINE decision

## User Management

### `sync_user`
```rust
#[spacetimedb::reducer]
pub fn sync_user(ctx: &ReducerContext, action: String, user_data: String)
```

**Purpose**: Synchronizes user accounts from Django solawispielplatz.

**Parameters**:
- `action`: Either "upsert" or "delete"
- `user_data`: JSON string containing `UserSyncData`

**Actions**:
- **"upsert"**: Creates or updates user account
  - Deletes existing account if it exists
  - Inserts new account with updated data
- **"delete"**: Removes user account from database

**UserSyncData Structure**:
```rust
{
    "mitgliedsnr": u64,         // Django user ID
    "name": Option<String>,     // User's display name
    "email": Option<String>,    // Primary email address
    "is_active": Option<bool>,  // Account active status
    "updated_at": Option<String> // Last update timestamp
}
```

## Category Management

### `add_message_category`
```rust
#[spacetimedb::reducer]
pub fn add_message_category(
    ctx: &ReducerContext,
    name: String,
    email_address: String,
    description: String,
) -> Result<(), String>
```

**Purpose**: Creates new email categories (mailing lists).

**Parameters**:
- `name`: Human-readable category name (e.g., "SoLaWi News")
- `email_address`: Category email address (e.g., "news@solawi.org")  
- `description`: Longer description of the category's purpose

**Authorization**: Checks `is_admin_user()` - returns error if not authorized.

**Behavior**:
- Creates new `MessageCategory` with `active: true`
- Auto-generates ID via `#[auto_inc]`
- Logs the creation with the creating identity

## Subscription Management

### `add_subscription`
```rust
#[spacetimedb::reducer]
pub fn add_subscription(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    subscriber_email: String,
    category_id: u64,
)
```

**Purpose**: Creates subscriptions linking users to email categories.

**Parameters**:
- `subscriber_account_id`: Links to `account.id`
- `subscriber_email`: Email address that will receive category emails
- `category_id`: Foreign key to `message_categories.id`

**Behavior**:
- Creates new `Subscription` with `active: true`
- Sets `subscribed_at` to current timestamp
- Auto-generates ID via `#[auto_inc]`

**Note**: Currently no authorization check - should be extended to verify the user can create subscriptions.

## IP Blocking Management

### `block_ip`
```rust
#[spacetimedb::reducer]
pub fn block_ip(ctx: &ReducerContext, ip: String, reason: String)
```

**Purpose**: Adds IP addresses to the spam protection blacklist.

**Parameters**:
- `ip`: IP address to block (string format)
- `reason`: Human-readable reason for blocking

**Behavior**:
- Creates new `BlockedIp` with `active: true`
- Sets `blocked_at` to current timestamp
- IP address becomes primary key

**Note**: Currently no authorization check - should be extended to admin-only access.

## Utility and Debug Reducers

### `get_mta_logs`
```rust
#[spacetimedb::reducer]
pub fn get_mta_logs(ctx: &ReducerContext)
```

**Purpose**: Debug function to output all MTA logs to the SpacetimeDB console.

**Behavior**:
- Iterates through all `mta_connection_log` entries
- Iterates through all `mta_message_log` entries
- Logs details to SpacetimeDB log output

**Usage**: Call via `spacetime call kommunikationszentrum get_mta_logs`

### `add_test_accounts`
```rust
#[spacetimedb::reducer]
pub fn add_test_accounts(ctx: &ReducerContext)
```

**Purpose**: Debug function to add sample user accounts for testing.

**Behavior**:
- Creates two test accounts with IDs 1 and 2
- Sets basic user information for testing purposes

**Note**: Should be removed or protected in production environments.

## Authorization Helper Functions

### `is_admin_user`
```rust
fn is_admin_user(_ctx: &ReducerContext) -> bool
```

**Purpose**: Determines if the current user has administrative privileges.

**Current Implementation**: Returns `true` for all authenticated users (demo purposes).

**Production Implementation Should**:
- Check JWT claims (e.g., `is_staff`, `is_superuser`, `groups`)
- Maintain whitelist of admin identities
- Implement role-based permissions

## Error Handling

All reducers follow these error handling patterns:

**JSON Parsing Errors**:
```rust
match serde_json::from_str::<Type>(&json_data) {
    Ok(data) => { /* process */ },
    Err(e) => { 
        log::error!("Failed to parse data: {}", e); 
        return; // or return Err(e.to_string())
    }
}
```

**Authorization Errors**:
```rust
if !is_admin_user(ctx) {
    return Err("Unauthorized: Admin access required".to_string());
}
```

**Logging Standards**:
- Use `log::info!()` for normal operations
- Use `log::warn!()` for suspicious but handleable events
- Use `log::error!()` for errors that prevent operation completion
- Always redact sensitive information (IPs, emails, etc.)

## Calling Reducers

### From Webhook Proxy
```rust
// Via SpacetimeDB SDK
db_connection.reducers.handle_mta_hook(hook_json)?;
db_connection.reducers.sync_user("upsert".to_string(), user_json)?;
```

### From Command Line
```bash
# Call reducer directly
spacetime call kommunikationszentrum get_mta_logs

# Call with parameters
spacetime call kommunikationszentrum add_message_category "News" "news@solawi.org" "Weekly newsletter"
```

### From Admin Interface
```rust
// Via SpacetimeDB client connection
client.add_message_category("Events".to_string(), "events@solawi.org".to_string(), "Event announcements".to_string()).await?;
```
