Comprehensive Architectural & Data Model Overview: Kommunikationszentrum
I have thoroughly investigated the kommunikationszentrum project (along with its counterpart solawispielplatz) across documentation, source code, database schemas, and configuration. Below are the findings organized by the six questions requested.

1. Overall Architecture
kommunikationszentrum is a specialized Community Supported Agriculture (SoLaWi) email management and mailing list system. It manages category-based mailing lists (e.g. distribution points / Verteilpunkte, working groups / Arbeitsgruppen, announcement lists) with subscriber access control, automated fan-out, delivery tracking, and self-service management.

The system is organized as a Rust workspace with three primary crates, cooperating with two external services:

server/ (SpacetimeDB Module):

Compiles to WebAssembly and runs inside SpacetimeDB.
Acts as the canonical database and core business logic engine.
Contains all table definitions, reducers (mutations/actions), scheduled background jobs (re-queuing retries, expiring stale worker leases), and row-level security views (visible_accounts, visible_subscriptions, visible_message_categories, etc.).
Exposes HTTP handlers for external webhooks:
/mta-hook: Ingests and inspects incoming emails from Stalwart MTA.
/user-sync: Ingests user identity and category assignment syncs from Django.
/mailing-list/unsubscribe: Handles RFC 8058 One-Click list unsubscriptions.
sender/ (SMTP Delivery Daemon):

Standalone asynchronous Tokio daemon.
Connects to SpacetimeDB via the SpacetimeDB Rust SDK (WebSocket).
Claims pending fan-out ingress jobs (mail_ingress) and decomposes each inbound email into personalized, RFC 5322–compliant outbound messages for each subscriber (using lettre).
Injects standard mailing list headers (List-Id, List-Post, List-Unsubscribe, List-Unsubscribe-Post, X-BeenThere, etc.) and unique one-click unsubscribe URLs.
Claims pending delivery messages (mail_delivery_pending), connects via SMTP using category-specific credentials (provisioned in Stalwart), sends the email, and handles transient/permanent errors with retry queues and backoff.
admin/ (Admin & Member Web Interface):

A single-page WebAssembly application built with Dioxus 0.6.
Authenticates via Django OAuth 2.0 / OpenID Connect (PKCE).
Connects to SpacetimeDB with the user's JWT ID token to subscribe reactively to real-time data views.
Member features: View public and subscribed private mailing lists, subscribe/unsubscribe with one click, view archived emails for subscribed categories.
Admin features: Create/provision categories (calling Stalwart JMAP to create mailboxes and app passwords), manage topics/tags, manage member subscriptions, monitor delivery pipeline status and error events.
External Services:

Django (solawispielplatz): Upstream master for user identity, membership numbers (mitgliedsnr), staff privileges, and vegetable share (Ernteanteil) pickup locations (Verteilpunkte). Acts as the OAuth 2.0 / OIDC identity provider and triggers sync webhooks.
Stalwart MTA: Enterprise mail transfer agent. Receives incoming mail via SMTP, validates sender/recipient via SpacetimeDB HTTP hooks, provides JMAP management APIs for mailbox provisioning, and serves as the outbound SMTP submission server.
2. How Users / Accounts Are Modeled
User data is defined in server/src/models/account.rs:

rust


pub struct Account {
    #[primary_key]
    pub id: u64,             // mitgliedsnr from Django
    #[unique]
    pub identity: Identity,  // SpacetimeDB Identity (32-byte cryptographic ID)
    pub name: String,        // Full name
    #[index(btree)]
    pub email: String,       // Primary email address
    pub is_active: bool,     // Whether account is active
    #[index(btree)]
    pub last_synced: Timestamp,
}
pub struct AdminIdentity {
    #[primary_key]
    pub identity: Identity,
}
pub struct WebhookToken {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub token_hash: String,       // BLAKE3 hash of bearer token
    pub label: String,
    pub permissions: Vec<String>, // e.g. ["sync-user", "mta-hook"]
    pub created_at: Timestamp,
    pub active: bool,
}
Primary Identifier: The member's integer ID from Django (mitgliedsnr) is used as Account.id.
Identity Derivation & Security: SpacetimeDB computes user identities deterministically from Django claims:
rust


let issuer_url = format!("{}{}", DJANGO_OAUTH_BASE_URL, DJANGO_OAUTH_ISSUER_PATH);
let identity = Identity::from_claims(&issuer_url, &mitgliedsnr.to_string());
When a user logs in via OAuth in the Dioxus UI, Django issues a JWT ID token where iss is the Django issuer and sub is mitgliedsnr. SpacetimeDB validates the JWT and binds the connection to this exact Identity.
Authorization & Public Views:
Tables in SpacetimeDB are private. Access is mediated by public views:
visible_accounts: Admins see all accounts; regular users see only their own Account row (identity == ctx.sender()).
visible_admin_identities: Checked to determine if a caller is an administrator.
3. How Email Addresses Relate to Users / Accounts
Single Primary Email Per Account:
An Account has exactly one email: String field.
In Django (solawispielplatz), User.username is an EmailField and User.email is a property returning username. There is no multi-email or alias table for users.
Denormalization in Subscriptions:
In the Subscription table, both subscriber_account_id: u64 and subscriber_email: String are stored. When synced from Django or created in the UI, subscriber_email is copied from Account.email.
Inbound Sender Authentication:
When an email arrives at Stalwart and triggers the SpacetimeDB hook (handle_data_stage), the system checks:
rust


let sender_account_id = ctx.db.account().email().filter(&from_address.to_string()).next().map(|a| a.id);
The sender's envelope.from address must match a registered Account.email. External senders not present in the account table are rejected (unless the sender is an admin).
Category Addresses:
Each MessageCategory has its own unique email_address: String (e.g. vp-reyerhof@solawis.de), which serves as the mailing list address.
4. How Topics, Categories, and Subscriptions Work
Found in server/src/models/category.rs and server/src/reducers/categories.rs:

Topics (Topic and MessageCategoryTopic)
Topic: { id: u64, name: String } (e.g., verteilpunkt, arbeitsgruppe).
MessageCategoryTopic: { id: u64, category_id: u64, topic_id: u64 } (many-to-many link).
Topics categorize mailing lists. In the Dioxus frontend, topics render as tabs (TopicTabButton), organizing categories for members, with an automatic fallback "Sonstige" tab for untagged categories.
Categories (MessageCategory)
Represents a mailing list topic:
rust


pub struct MessageCategory {
    pub id: u64,
    pub name: String,
    pub email_address: String,
    pub description: String,
    pub active: bool,
    pub visibility: CategoryVisibility, // Public | Private
    pub app_password_id: Option<u64>,  // FK to CategoryAppPassword
}
Visibility:
Public: Visible to all authenticated members; anyone can self-subscribe.
Private: Only visible to admins and to members who are already subscribed (e.g. specific working groups or distribution points).
Provisioning:
Admins can provision categories via the SpacetimeDB procedure provision_message_category. This makes an outbound JMAP call to Stalwart to create the mailbox account, generates an SMTP app password, and saves both the category and credentials in SpacetimeDB.
Subscriptions (Subscription)
Links an account to a category:
rust


pub struct Subscription {
    pub id: u64,
    pub subscriber_account_id: u64,
    pub subscriber_email: String,
    pub category_id: u64,
    pub subscribed_at: Timestamp,
    pub status: SubscriptionStatus,
}
Subscription Lifecycle / Statuses:
AutomaticallySubscribed: Created via automated sync from Django.
AutomaticallyUnsubscribed: Deactivated via automated sync from Django.
ManuallySubscribed: Subscribed explicitly by the user or admin in the web UI.
ManuallyUnsubscribed: Unsubscribed explicitly by the user or admin.
LinkUnsubscribed: Unsubscribed via the one-click List-Unsubscribe link in an email.
RequiredSubscribed: Mandatory subscription assigned by Django (e.g., a member's assigned pickup point / Verteilpunkt).
Protection Rules & Invariants:
Sync Protection: Django sync will not overwrite ManuallySubscribed, ManuallyUnsubscribed, or LinkUnsubscribed.
Required Subscription Protection: A user cannot unsubscribe from a RequiredSubscribed category in the UI, nor can they use the one-click unsubscribe link. Only an admin or Django sync (when the member's Verteilpunkt assignment ends) can remove it.
Unsubscribe Tokens (SubscriptionUnsubscribeToken)
Each subscription receives a cryptographic token: sub-{subscription_id}-{32_hex_random}.
Active tokens are linked to the subscription to enable RFC 8058 One-Click unsubscription.
5. Mailing List and Communication Functionality
The end-to-end communication flow works as follows:

Inbound Mail Ingestion & Access Control:

Stalwart MTA receives an inbound email and triggers the SpacetimeDB /mta-hook HTTP handler at each SMTP stage (Connect, Ehlo, Mail, Rcpt, Data).
In the Data stage:
The target category email address is verified against active MessageCategory records.
The sender's From address is matched against Account.email.
Posting Permission: The sender must either be an admin OR have an active subscription (s.category_id == cat_id && s.status.is_active()) to that specific category. External and unsubscribed senders are rejected.
If authorized, the message is stored in MailMessage and ReceivedMessage, and a fan-out job is enqueued in MailIngress.
Outbound Fan-Out & Message Assembly (sender):

The sender daemon claims MailIngress jobs under a lease.
For each active subscriber of the category:
Prepares an RFC 5322 message using lettre.
Rewrites the Subject to prepend [ListName]: (preserving and standardizing Re: and Fwd: tags).
Rewrites From to the mailing list address (vp-reyerhof@...).
Sets Reply-To to the original sender (poster@...).
Sets To to the subscriber's address.
Injects list headers:
List-Id: <ListName <list@domain>>
List-Post: <mailto:list@domain>
List-Unsubscribe: <mailto:list@domain?subject=unsubscribe>, <https://.../unsubscribe?token=...>
List-Unsubscribe-Post: List-Unsubscribe=One-Click
Precedence: list
X-Mailing-List: ListName
X-BeenThere: list@domain
Stores the rendered raw message in MailDeliveryMessage and queues a work item in MailDeliveryPending.
Delivery Execution & Fault Tolerance:

sender claims pending items (MailDeliveryClaimed) with a lease expiration.
Resolves category-specific SMTP credentials from CategoryAppPassword.
Sends the email to Stalwart / SMTP relay.
Final status is saved in MailDeliveryDone (Sent, Failed, Bounced, Cancelled).
Transient failures are moved to MailDeliveryTemporaryFailed with a delay timestamp. A scheduled SpacetimeDB reducer (requeue_temporary_failed_mails) automatically moves them back to MailDeliveryPending when ready.
A scheduled reducer (expire_stale_delivery_claims) detects and recovers stranded jobs if a sender instance crashes.
Archiving & Reading:

Delivered messages are queryable by subscribers through the visible_messages view and rendered in the MessagesPage of the Dioxus web interface.
6. Integration with the Solawis Django Project (solawispielplatz)
The integration between kommunikationszentrum and solawispielplatz is already implemented and operational across identity, authorization, and data sync:

OAuth 2.0 / OpenID Connect Provider:

Django acts as the central OAuth authorization server using django-oauth-toolkit (endpoints at /o/authorize/ and /o/token/).
Supports Authorization Code Flow with PKCE for the Dioxus public frontend.
Provides JWT ID tokens with standard and custom claims (sub = mitgliedsnr, iss = http://127.0.0.1:8000/o, email, is_staff, is_superuser).
Automated User & Verteilpunkt Synchronization:

In Django (src/mitgliederverwaltung/signals.py), post_save and post_delete signals on User and Ernteanteil models trigger webhook syncs to SpacetimeDB (POST /v1/database/kommunikation/route/user-sync).
Authenticated via HTTP Bearer token matching a WebhookToken record in SpacetimeDB with sync-user permission.
Sync payload structure:
json


{
  "action": "upsert",
  "user": {
    "mitgliedsnr": 1234,
    "name": "Max Mustermann",
    "email": "max@example.org",
    "is_active": true,
    "is_admin": false,
    "updated_at": "2024-01-01T12:00:00Z",
    "categories": [
      {
        "name": "VP Reyerhof",
        "email_address": "vp-reyerhof@example.org",
        "description": "Verteilpunkt Reyerhof",
        "required": true
      }
    ],
    "unsubscribe_category_emails": ["vp-old@example.org"]
  }
}
Verteilpunkt Mapping: In Django, Verteilpunkt has a mailingliste field (models.EmailField). When a member holds a share at a Verteilpunkt, Django marks the category subscription as required: True. SpacetimeDB assigns SubscriptionStatus::RequiredSubscribed. If a member changes pickup locations, Django sends the old address in unsubscribe_category_emails, which deactivates it.
Resilience: If SpacetimeDB is down during a Django update, failed requests are stored in a Django cache retry queue (SPACETIME_RETRY_QUEUE_KEY) and retried.
Bulk Sync Command: Django includes a management command python src/manage.py sync_users_to_spacetimedb to seed or re-sync the entire user base.
