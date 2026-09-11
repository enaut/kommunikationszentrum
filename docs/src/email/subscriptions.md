# Subscription System

The subscription system manages which users can send emails to which categories in the Kommunikationszentrum. It provides fine-grained access control and ensures that only authorized senders can distribute content to email lists.

## Overview

The subscription system enables:

- **User-Category Mapping**: Links users to email categories they can send to
- **Access Control**: Prevents unauthorized email distribution
- **Flexible Management**: Users can be subscribed/unsubscribed from categories
- **Audit Trail**: Complete history of subscription changes

## Subscription Model

### Database Schema

Subscriptions are stored in the `subscriptions` table:

```rust
#[spacetimedb::table(name = subscriptions)]
pub struct Subscription {
    #[primarykey]
    #[autoinc]
    pub id: u64,
    pub subscriber_account_id: u64,
    pub account_email_id: u64,
    pub category_id: u64,
    pub subscribed_at: i64,
    pub status: SubscriptionStatus,
    pub permission: SubscriptionPermission,
}
```

### Relationship Diagram

```d2
{{#include subscriptions-relationships.d2}}
```

## Subscription Management

#### Via Admin Interface

Users with admin privileges can manage subscriptions through the web interface:

1. Select user account
2. Choose categories to subscribe to  
3. Confirm subscription creation

#### Via User Self-Service

Regular users can manage their own subscriptions:

1. Login to personal dashboard
2. View available categories
3. Subscribe/unsubscribe as desired
4. Changes take effect immediately

## Email Processing with Subscriptions

### Subscription Checking Flow

During the DATA stage of MTA processing, the system validates that senders are subscribed to target categories:

```d2
{{#include subscriptions-checking-flow.d2}}
```
