# Email Categories

Email categories form the backbone of the Kommunikationszentrum's email routing system. They define which email addresses are valid recipients and how emails should be processed based on their destination.

## Overview

The email category system enables:

- **Organized Email Lists**: Different topics (news, events, general) as separate categories
- **Targeted Distribution**: Users can subscribe to specific categories of interest  
- **Access Control**: Only valid categories accept incoming emails
- **Administrative Flexibility**: Categories can be activated/deactivated as needed

## Category Structure

### Database Schema

Email categories are stored in the `message_categories` table:

```rust
#[spacetimedb::table(name = message_categories)]
pub struct MessageCategory {
    #[primary_key]
    #[auto_inc]
    pub id: u64,                // Auto-increment primary key
    pub name: String,           // Human-readable category name  
    pub email_address: String,  // Email address for this category
    pub description: String,    // Description of the category
    pub active: bool,           // Whether category is currently active
}
```

### Visual Representation

```d2
{{#include categories-visibility.d2}}
```

## Category Management

### Creating Categories

Categories can be created through the admin interface.

**Parameters:**
- `name`: Human-readable category name (e.g., "SoLaWi News")
- `email_address`: Unique email address for the category
- `description`: Detailed description of the category's purpose

**Authorization**: Only admin users can create categories.

### Deactivating Categories

Categories can be temporarily or permanently deactivated:

```bash
# Deactivate a category (SQL-like syntax)
spacetime sql "UPDATE message_categories SET active = false WHERE id = 3"
```

**Effects of deactivation:**
- New emails to the category are rejected
- Existing subscriptions remain but are inactive
- Category still appears in admin interface
- Can be reactivated later

## Email Address Validation

### Format Requirements

Category email addresses must follow these rules:

1. **Valid email format**: `name@domain.tld`
2. **Unique addresses**: No two categories can share the same email
3. **Domain consistency**: Typically use the same domain (e.g., `@solawis.de`). The domain must be configured in stalwart.
4. **Descriptive names**: Use meaningful prefixes (`news@`, `events@`, etc.)

### Domain Configuration

For proper email routing, ensure:

1. **DNS MX records** point to your Stalwart MTA server
2. **Virtual domain** configuration includes category domains
3. **Wildcard routing** or explicit address mapping in MTA

## Category Processing Logic

### Validation Flow

```d2
{{#include categories-validation-flow.d2}}
```

## Integration with Subscriptions

Categories work closely with the subscription system:

### Subscription Relationship

```d2
{{#include categories-subscription-relationship.d2}}
```
### Debugging Commands

```bash
# List all categories
spacetime sql "SELECT * FROM message_categories"

# Check category activity status  
spacetime sql "SELECT name, email_address, active FROM message_categories WHERE active = false"

# Find categories without subscriptions
spacetime sql "
SELECT mc.name, mc.email_address 
FROM message_categories mc
LEFT JOIN subscriptions s ON mc.id = s.category_id
WHERE s.id IS NULL"

# Recent category processing
spacetime sql "SELECT * FROM mta_connection_log WHERE stage = 'RCPT' ORDER BY timestamp DESC LIMIT 10"
```
