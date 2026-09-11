# Subscription System

The subscription system manages membership and delivery routing for mailing lists in the Kommunikationszentrum. It provides fine-grained access control, recipient distribution, and permission separation between reading and posting.

## Overview

The subscription system enables:

- **Multi-Email per Account**: Accounts can register multiple email addresses (`AccountEmail`), each verified independently.
- **Shared Email Support**: Multiple accounts can share the same email address (e.g. family or organizational accounts), with indexed lookup `(account_id, email)`.
- **Multi-Email Subscriptions**: An account can maintain distinct active subscriptions to the same category for different email addresses.
- **Role-Based Permissions**: Subscriptions support granular `Read` and `Write` permissions.
- **Sync & Requirement Protection**: Distinguishes between automated Django syncs, mandatory pickup-point requirements (`RequiredSubscribed`), and explicit user/admin actions.
- **One-Click Unsubscribe**: Cryptographic tokens (`SubscriptionUnsubscribeToken`) support RFC 8058 one-click unsubscription.

---

## Subscription Model

### Database Schema

Subscriptions are stored in the `subscriptions` table and link an account and specific email address to a category:

```rust
#[spacetimedb::table(accessor = subscriptions)]
pub struct Subscription {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub subscriber_account_id: u64,
    #[index(btree)]
    pub account_email_id: u64,
    #[index(btree)]
    pub category_id: u64,
    pub subscribed_at: Timestamp,
    #[index(btree)]
    pub status: SubscriptionStatus,
    pub permission: SubscriptionPermission,
}
```

Linked email addresses are stored in `account_emails`:

```rust
#[spacetimedb::table(accessor = account_emails)]
pub struct AccountEmail {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub account_id: u64,
    #[index(btree)]
    pub email: String,
    pub source: EmailSource, // DjangoSync | Native
    pub is_verified: bool,
    #[index(btree)]
    pub added_at: Timestamp,
}
```

### Relationship Diagram

```d2
{{#include subscriptions-relationships.d2}}
```

---

## Subscription Status Lifecycle

| Status | Origin | Active? | Sync Can Deactivate? | User Can Remove? |
|---|---|---|---|---|
| `AutomaticallySubscribed` | Django webhook sync | Yes | Yes (`AutomaticallyUnsubscribed`) | Yes (`ManuallyUnsubscribed`) |
| `AutomaticallyUnsubscribed` | Django webhook sync | No | No (sync can reactivate) | Yes (`ManuallySubscribed`) |
| `RequiredSubscribed` | Django sync (e.g. assigned Verteilpunkt) | Yes | Only when Django removes assignment | No (protected) |
| `ManuallySubscribed` | User or Admin via UI | Yes | No (protected from sync overrides) | Yes |
| `ManuallyUnsubscribed` | User or Admin via UI | No | No (protected from sync overrides) | Yes |
| `LinkUnsubscribed` | RFC 8058 One-Click link | No | No (protected from sync overrides) | Yes (can resubscribe in UI) |

---

## Permissions: Read vs. Write

Every subscription has an associated `SubscriptionPermission`:

- **`SubscriptionPermission::Read`** (Default): The subscriber receives all emails distributed by the category mailing list, but is not authorized to post to the list unless they are an administrator.
- **`SubscriptionPermission::Write`**: The subscriber receives all category emails **and** is authorized to send emails to the category address via SMTP through the MTA hook.

Administrators can adjust a subscription's permission at any time using the `update_subscription_permission` reducer.

---

## Subscription Validation & Security Rules

When a user or admin calls `add_subscription`:
1. **Ownership**: Non-admin callers can only manage subscriptions for their own account (`subscriber_account_id`).
2. **Email Verification**: Non-admin callers cannot subscribe an unverified email address (`email.is_verified == true` required).
3. **Category Visibility**: Non-admin callers can self-subscribe only to `CategoryVisibility::Public` categories. Private categories require an existing subscription or admin intervention.
4. **Account Scoping**: The specified `account_email_id` must belong to `subscriber_account_id`.

---

## Lifecycle Cleanup & Cascading

The system guarantees that dangling subscriptions or orphan tokens are never left behind:

- **Email Removal (`remove_account_email`)**: Removing a linked email cascades deletion to all subscriptions tied to that `account_email_id` and revokes their associated `SubscriptionUnsubscribeToken` rows.
- **Django Sync Email Removal**: If an email is removed from the Django sync payload:
  - If the user already has a subscription to that category on their `primary_email_id`, the redundant subscription and its token are deleted.
  - If no subscription exists on `primary_email_id`, the subscription is migrated: `sub.account_email_id = primary_email_id`.
- **Account Deletion (`action = "delete"`)**: Deleting an account cascades across all subscriptions, unsubscribe tokens, linked emails, verification tokens, and admin identities.

---

## User & Admin Interface

### Member Subscriptions Page

In the Web UI, each category card provides an **inline checkbox list** containing all verified and unverified email addresses linked to the user's account:

- **Instant Toggle**: Checking or unchecking an email address immediately invokes `add_subscription` or `remove_subscription`.
- **Unverified Badge**: Unverified addresses are disabled with an `Unverified` badge and cannot be subscribed until verified.
- **Required Badge**: Mandatory subscriptions (`RequiredSubscribed`) display a `Required` badge and disabled checkbox to prevent accidental removal.
- **Subscribed Badge**: The category card header displays a green `Subscribed` badge whenever at least one email address of the account is active in that category.

### Member Search & Administration

Administrators can manage subscriptions across all accounts:
- **Member Search Bar**: Real-time filtering by Member Number (`account.id`), Name (`account.name`), or any linked email address (`account_emails`).
- **Counter Badge**: Live display of `{filtered} / {total}` members.
- **Category Detail Modal**: Add Subscriber modal allows searching members across names, IDs, and email addresses, and displays accounts that have at least one unsubscribed email available.

---

## Email Processing with Subscriptions

### Subscription Checking Flow

During the DATA stage of MTA processing, the system validates that senders are authorized to distribute mail to target categories:

```d2
{{#include subscriptions-checking-flow.d2}}
```
