# User Permissions

The permission system in Kommunikationszentrum implements role-based access control through JWT claims and SpacetimeDB reducer-level authorization. User roles and permissions are managed in Django and enforced throughout the system.

## Permission Levels

### Regular Users

Regular users have access to basic functionality:

**Subscription Management**: Users can view and modify their own email topic subscriptions.

**Personal Data**: Access to view and update their own account information.

**Topic Browsing**: View available email topics and their descriptions.

Regular users are identified by the absence of administrative flags in their JWT claims (`is_staff: false`, `is_superuser: false`) and lack of `SubscriptionPermission::Moderate` on any topic.

### Topic Moderators

Subscribed users granted `SubscriptionPermission::Moderate` on specific topics:

**Topic Management**: Access to the "Themen" (Topics) view in the admin interface, scoped exclusively to their moderated topics.

**Topic Metadata**: Capability to update the title, description, and visibility of their assigned topics via `update_message_topic`. They may adjust default permission between `Read` and `Write`, but setting or removing `Moderate` default permissions is Admin-only.

**Category Assignments**: Capability to assign or remove category tags on their assigned topics via `set_topic_categories`.

**Subscriber Roster & Privacy-Preserving Search**: Visibility into active subscribers for their assigned topics (`context_accounts`, `visible_account_emails`, and `visible_subscriptions`). For adding new subscribers, topic moderators query `subscriber_search_candidates` with a search term of at least 5 characters; this returns only minimal candidate details (`account_id`, `name`, `account_email_id`, `email`) and excludes unverified or already-subscribed accounts, preserving member privacy.

**Subscriber Management**: Capability to add new subscribers (`add_subscription` / `admin_add_subscription`), remove subscribers (`remove_subscription`), and toggle subscriber permissions between `Read` and `Write` (`update_subscription_permission`). Non-admin moderators can only assign or remove `ManuallySubscribed` or `ManuallyUnsubscribed` statuses.

**Mailing List Ingress**: Authorized to send emails to the topic address via SMTP through the MTA hook (`Moderate` implies `Write`).

**Strict Boundaries**:
- Cannot create new topics (`add_message_topic` / `provision_message_topic` is Admin-only).
- Cannot delete topics (`remove_message_topic` is Admin-only).
- Cannot set or remove `Moderate` default permission on topics (`update_message_topic` default permission to/from `Moderate` is Admin-only).
- Cannot grant or revoke `Moderate` rights (`update_subscription_permission` to/from `Moderate` is Admin-only).
- Cannot unsubscribe or modify other moderators on the topic.
- Cannot set, modify, or remove `RequiredSubscribed` subscriptions.

### Staff Users

Django staff members have elevated permissions:

**User Management**: View and modify other user accounts and subscriptions.

**Topic & Category Management**: Create, modify, and deactivate email topics and categories.

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

These claims are embedded in the JWT ID token. SpacetimeDB checks the token signature, issuer, subject, and expiry. It does not authorize reducers from `is_staff`, `is_superuser`, or `groups`. Admin access in the module is the `admin_identities` row written by user sync.

### Reducer-Level Authorization

SpacetimeDB reducers implement authorization checks using helper functions defined in `common/auth.rs`:

```rust
pub fn is_admin_user(ctx: &(impl CtxDbRead + CtxWithSender)) -> bool {
    is_admin_identity(ctx, ctx.sender())
}

pub fn is_topic_moderator(ctx: &(impl CtxDbRead + CtxWithSender), topic_id: u64) -> bool {
    // True if sender has an active subscription with SubscriptionPermission::Moderate
}

pub fn is_admin_or_topic_moderator(ctx: &(impl CtxDbRead + CtxWithSender), topic_id: u64) -> bool {
    is_admin_user(ctx) || is_topic_moderator(ctx, topic_id)
}
```

**Topic Management Example**:
```rust
#[spacetimedb::reducer]
pub fn update_message_topic(
    ctx: &ReducerContext,
    topic_id: u64,
    name: String,
    description: String,
    visibility: Option<TopicVisibility>,
    default_permission: Option<SubscriptionPermission>,
    clear_provisioning_lock: Option<bool>,
) -> Result<(), String> {
    if !is_admin_or_topic_moderator(ctx, topic_id) {
        return Err("Unauthorized: Admin or topic moderator access required".to_string());
    }
    // Update topic logic...
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

**Topic Editors**: Users who can create and modify email topics and categories  
**Subscription Managers**: Users who can manage subscriptions for other users  
**System Administrators**: Users with full system access

Group memberships are included in JWT tokens via the `groups` claim and can be used for specific authorization decisions.
