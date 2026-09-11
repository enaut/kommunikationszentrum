# Subscription Management

The Kommunikationszentrum web interface provides self-service subscription management for members and administrative tools for list operators.

---

## 1. Member Self-Service (Subscriptions Page)

Authenticated members manage their mailing list preferences on the **Abonnements** (Subscriptions) page:

- **Topic Tabs**: Categories are organized into topic tabs (e.g. *Verteilpunkt*, *Arbeitsgruppe*, and untagged *Sonstige*).
- **Inline Email Checkboxes**: Each category card displays an inline checklist of all email addresses linked to the member's account.
  - **Multi-Email Subscriptions**: Members can subscribe one, several, or all of their linked addresses to a category independently.
  - **Instant Toggle**: Clicking a checkbox immediately triggers `add_subscription` or `remove_subscription` via SpacetimeDB WebSockets—no "Save" button required.
- **Status & Verification Badges**:
  - `Subscribed` (Green header badge): Appears on the category card whenever at least one email of the account is actively subscribed.
  - `Required` (Blue badge): Mandatory subscriptions (such as assigned vegetable share pickup points synced from Django) are checked and disabled to prevent accidental unsubscription.
  - `Unverified` (Orange badge): Email addresses awaiting verification cannot be subscribed until confirmed via the verification token email.

---

## 2. Admin Member Management (Members Page)

Administrators manage member records, linked email addresses, and subscription statuses from the **Mitglieder** (Members) view:

- **Live Member Search Bar**: A live search bar filters members in real-time across:
  - Member Number (`account.id`)
  - Member Name (`account.name`)
  - Any linked primary or alternative email address (`account_emails`)
- **Filtered Counter Badge**: Displays `{filtered} / {total}` (e.g. `12 / 150 Mitglieder`), updating dynamically as search queries are entered.
- **Empty State Feedback**: If no members match the query, a clear notification banner is shown.
- **Account Email Inspection**: Admins can see all linked email addresses per member, their verification status (`is_verified`), and source (`DjangoSync` vs `Native`).
- **Adding Member Emails**: Admins can directly register and pre-verify addresses for any account using `admin_add_account_email`.

---

## 3. Category Detail & Subscriber Administration

Within each category's detail view, administrators manage subscribers and permission tiers:

- **Subscribers Table**: Lists every active subscriber, including:
  - Member Name & Account ID
  - Subscribed Email Address
  - Subscription Status (`AutomaticallySubscribed`, `ManuallySubscribed`, `RequiredSubscribed`, etc.)
  - Permission Level (`Read` vs `Write`)
- **Permission Management**:
  - Admins can toggle permissions between `Read` (receive list broadcasts only) and `Write` (authorized to post to the list via MTA).
  - Toggling invokes the `update_subscription_permission` reducer.
- **Adding Subscribers**:
  - The **+ Abonnent hinzufügen** modal provides a search filter across member names, numbers, and emails.
  - **Smart Filtering**: The member selector only shows members who have at least one verified email not yet subscribed to the category.
  - Admins can select an explicit initial status (`SubscriptionStatus`) and permission via `admin_add_subscription`.
- **Removing Subscribers**:
  - Admins can deactivate any subscription, invoking `remove_subscription` (which transitions the subscription to `ManuallyUnsubscribed` and deactivates the associated one-click unsubscribe token).
