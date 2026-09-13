# Email Topics & Categories

Email topics and categories form the backbone of the Kommunikationszentrum's email routing system:
- **Message Topics (`MessageTopic`)** represent the mailing list channels (defining incoming/outgoing email addresses, subscribers, Stalwart app passwords, and delivery permissions).
- **Categories (`Category`)** represent reusable classification tags (e.g., *Verteilpunkt*, *Arbeitsgruppe*) used to filter and organize topics.
- **MessageTopicCategory (`MessageTopicCategory`)** connects topics to categories in a flexible many-to-many relationship.

---

## Overview

The email topic and category system enables:

- **Organized Email Lists**: Distinct mailing lists (news, events, distribution points) represented as individual message topics.
- **Category Classification**: Topics can be assigned one or more categories for multi-dimensional filtering in the member and admin interfaces.
- **Granular Access & Permissions**: Topics define whether regular subscribers have `Read` (broadcast-only) or `Write` (can post to the list) privileges.
- **Targeted Distribution**: Users subscribe their verified email addresses to specific topics of interest.
- **Mailbox & Credential Isolation**: Each topic provisioned in Stalwart possesses its own mailbox and SMTP application password (`TopicAppPassword`).
- **Visibility Control**: Topics can be `Public` (visible to all members) or `Private` (visible only to admins and invited/subscribed members).

---

## Data Model & Schema

### Database Tables

Topics, categories, and their associations are defined in `server/src/models/topic.rs`:

```rust
/// Determines who can discover a message topic in the member-facing view.
#[derive(SpacetimeType, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TopicVisibility {
    #[default]
    Public,
    Private,
}

#[derive(SpacetimeType, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubscriptionPermission {
    #[default]
    Read,
    Write,
}

/// The core mailing list entity.
#[spacetimedb::table(accessor = message_topics)]
pub struct MessageTopic {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub name: String,
    #[unique]
    pub email_address: String,
    pub description: String,
    pub active: bool,
    #[index(btree)]
    pub visibility: TopicVisibility,
    #[index(btree)]
    pub app_password_id: Option<u64>,
    pub default_permission: SubscriptionPermission,
    pub locked_is_provisioning: bool,
}

/// Dedicated Stalwart SMTP application passwords for each topic.
#[spacetimedb::table(accessor = topic_app_passwords)]
pub struct TopicAppPassword {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub secret: String,
    #[index(btree)]
    pub stalwart_id: String,
    pub created_at: Timestamp,
}

/// A reusable category tag, such as `Verteilpunkt` or `Arbeitsgruppe`.
#[spacetimedb::table(accessor = categories)]
pub struct Category {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub name: String,
}

/// Many-to-many assignment of categories to message topics.
#[spacetimedb::table(accessor = message_topic_categories)]
pub struct MessageTopicCategory {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub topic_id: u64,
    #[index(btree)]
    pub category_id: u64,
}
```

### Visual Representation

```d2
{{#include categories-visibility.d2}}
```

---

## Topic & Category Management

### Provisioning Topics

Topics can be created and provisioned directly through the Admin Web Interface or via procedures:

1. **Admin Web Interface**: Navigate to **Topics → Neuer Topic**. Enter the name, local email part, choose the domain from the synchronized domains dropdown, add a description, and select the visibility.
2. **SpacetimeDB Procedure (`provision_message_topic`)**:
   - Validates admin authorization.
   - Looks up the target domain in the `domains` table.
   - Calls Stalwart MTA's JMAP API to create the mailbox and generate a unique application password.
   - Persists the credentials to `topic_app_passwords` and inserts the `MessageTopic` record.

### Category Tags & Organization

- Topics can be assigned one or more categories in the topic detail view or during synchronization via `set_topic_categories`.
- Categories can be renamed globally across all tagged topics using the `rename_category` reducer.
- In the self-service Subscriptions view, members see category tabs that allow them to filter and navigate topics by their assigned tags.

### Deactivating Topics

Topics can be deactivated without deleting historical records:
- Incoming emails to deactivated topics are rejected at the RCPT stage (HTTP 550).
- Existing subscriptions remain in the database but will not receive or route mail.
- Can be reactivated by administrators at any time.

---

## Email Address Validation & Routing

### Format Requirements

Topic email addresses must follow these rules:
1. **Valid email format**: `name@domain.tld`
2. **Unique address**: No two topics can share the same email address.
3. **Synchronized domain**: The domain must be registered in Stalwart and synchronized into the SpacetimeDB `domains` table (see [Domain Synchronization](./stalwart-setup.md#domain-synchronization)).
4. **Descriptive names**: Use meaningful list identifiers (`news@solawis.de`, `vp-reyerhof@solawis.de`).

### Topic Validation Flow

```d2
{{#include categories-validation-flow.d2}}
```

---

## Integration with Subscriptions

Topics are the entity to which members subscribe:

```d2
{{#include categories-subscription-relationship.d2}}
```

### Debugging Commands

```bash
# List all message topics
spacetime sql kommunikation "SELECT id, name, email_address, active, visibility FROM message_topics"

# List all categories (classification tags)
spacetime sql kommunikation "SELECT * FROM categories"

# List topic to category assignments
spacetime sql kommunikation "SELECT * FROM message_topic_categories"

# Find inactive topics
spacetime sql kommunikation "SELECT name, email_address FROM message_topics WHERE active = false"

# Find topics without any subscribers
spacetime sql kommunikation "
SELECT mt.name, mt.email_address 
FROM message_topics mt
LEFT JOIN subscriptions s ON mt.id = s.topic_id
WHERE s.id IS NULL"

# Recent MTA hook processing for RCPT stage
spacetime sql kommunikation "SELECT * FROM mta_connection_log WHERE stage = 'rcpt' ORDER BY timestamp DESC LIMIT 10"
```
