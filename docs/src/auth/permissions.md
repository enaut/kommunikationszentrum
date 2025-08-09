# User Permissions

The permission system in Kommunikationszentrum implements role-based access control through JWT claims and SpacetimeDB reducer-level authorization. User roles and permissions are managed in Django and enforced throughout the system.

## Permission Levels

### Regular Users

Regular users have access to basic functionality:

**Subscription Management**: Users can view and modify their own email category subscriptions.

**Personal Data**: Access to view and update their own account information.

**Category Browsing**: View available email categories and their descriptions.

Regular users are identified by the absence of administrative flags in their JWT claims (`is_staff: false`, `is_superuser: false`).

### Staff Users

Django staff members have elevated permissions:

**User Management**: View and modify other user accounts and subscriptions.

**Category Management**: Create, modify, and deactivate email categories.

**System Monitoring**: Access to MTA logs and system status information.

Staff users are identified by the `is_staff: true` claim in their JWT tokens.

### Superusers

Django superusers have full administrative access:

**Complete System Control**: All staff permissions plus system configuration access.

**Advanced Operations**: Database maintenance operations and advanced debugging tools.

**Security Management**: IP blocking list management and security configuration.

Superusers are identified by the `is_superuser: true` claim in their JWT tokens.

## Authorization Implementation

### JWT Claims Processing

Django populates JWT tokens with permission-relevant claims during the OAuth flow:

```rust
#[derive(Debug, Serialize, Deserialize)]
struct UserInfoResponse {
    sub: String,                    // User ID
    preferred_username: String,     // Username
    email: Option<String>,
    is_staff: Option<bool>,         // Staff permission flag
    is_superuser: Option<bool>,     // Superuser permission flag
    groups: Option<Vec<String>>,    // Django group memberships
}
```

These claims are embedded in the JWT ID token and validated by SpacetimeDB on connection establishment.

### Reducer-Level Authorization

SpacetimeDB reducers implement authorization checks using helper functions:

```rust
fn is_admin_user(_ctx: &ReducerContext) -> bool {
    // Current implementation allows all authenticated users
    // Production should check JWT claims:
    // - is_staff or is_superuser flags
    // - specific group memberships
    // - maintain admin identity whitelist
    true
}
```

**Category Management Example**:
```rust
#[spacetimedb::reducer]
pub fn add_message_category(
    ctx: &ReducerContext,
    name: String,
    email_address: String,
    description: String,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    // Category creation logic...
    Ok(())
}
```

### Admin Interface Permission Checks

The admin interface respects user permissions for UI rendering:

**Conditional Features**: Administrative functions are only displayed for users with appropriate permissions.

**API Protection**: Backend reducers validate permissions regardless of UI state.

**Real-time Updates**: Permission changes are reflected immediately through SpacetimeDB subscriptions.

## Group-Based Permissions

### Django Groups Integration

Django groups provide fine-grained permission control:

**Category Editors**: Users who can create and modify email categories
**Subscription Managers**: Users who can manage subscriptions for other users  
**System Administrators**: Users with full system access

Group memberships are included in JWT tokens via the `groups` claim and can be used for specific authorization decisions.
