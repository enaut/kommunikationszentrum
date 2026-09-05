use spacetimedb::{SpacetimeType, Timestamp};

/// Determines who can discover a message category in the member-facing view.
#[derive(SpacetimeType, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CategoryVisibility {
    #[default]
    Public,
    Private,
}

impl CategoryVisibility {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "public" | "Public" => Ok(Self::Public),
            "private" | "Private" => Ok(Self::Private),
            _ => Err(format!(
                "Invalid category visibility '{value}'; expected 'public' or 'private'"
            )),
        }
    }
}

// Private: clients never subscribe to this table directly. `visible_message_categories`
// is the way clients read category rows.
#[spacetimedb::table(accessor = message_categories)]
pub struct MessageCategory {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub name: String,
    #[unique]
    pub email_address: String,
    pub description: String,
    pub active: bool,
    /// Controls whether regular members can see this category. Admins can see both variants.
    #[index(btree)]
    #[default(CategoryVisibility::Public)]
    pub visibility: CategoryVisibility,
    /// Stalwart app password for SMTP submission from this category mailbox.
    /// Set during `provision_message_category`; absent for DB-only categories.
    #[index(btree)]
    #[default(None::<u64>)]
    pub app_password_id: Option<u64>,
}

// Private: clients never subscribe to this table directly. `visible_category_app_passwords`
// is the way clients read app-password rows.
#[spacetimedb::table(accessor = category_app_passwords)]
pub struct CategoryAppPassword {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    /// Plaintext Stalwart app password, returned only on creation.
    pub secret: String,
    /// Stalwart credential id for future revocation via JMAP.
    #[index(btree)]
    pub stalwart_id: String,
    pub created_at: Timestamp,
}

/// A reusable category tag, such as `verteilpunkt` or `arbeitsgruppe`.
#[spacetimedb::table(accessor = topics)]
pub struct Topic {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub name: String,
}

/// Many-to-many assignment of topics to message categories.
#[spacetimedb::table(accessor = message_category_topics)]
pub struct MessageCategoryTopic {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub category_id: u64,
    #[index(btree)]
    pub topic_id: u64,
}

/// Category data as sent by the Django user-sync webhook for a single
/// mailing-list assignment (e.g. a Verteilpunkt).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CategorySyncData {
    pub name: String,
    pub email_address: String,
    pub description: String,
    #[serde(default = "default_category_visibility")]
    pub visibility: String,
    /// `None` leaves an existing category's topics unchanged. An empty array
    /// explicitly removes all of its topic assignments.
    #[serde(default)]
    pub topics: Option<Vec<String>>,
    #[serde(default)]
    pub required: bool,
}

pub fn default_category_visibility() -> String {
    "public".to_string()
}

/// Lifecycle status of a [`Subscription`].
#[derive(SpacetimeType, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionStatus {
    AutomaticallySubscribed,
    AutomaticallyUnsubscribed,
    ManuallySubscribed,
    ManuallyUnsubscribed,
    LinkUnsubscribed,
    RequiredSubscribed,
}

impl SubscriptionStatus {
    /// Whether a subscription with this status should currently receive mail.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::AutomaticallySubscribed | Self::ManuallySubscribed | Self::RequiredSubscribed
        )
    }

    /// Whether this status was set by the automatic sync path.
    pub fn is_automatic(&self) -> bool {
        matches!(
            self,
            Self::AutomaticallySubscribed | Self::AutomaticallyUnsubscribed
        )
    }

    /// Whether this state is controlled by the Django synchronization path.
    pub fn is_sync_managed(&self) -> bool {
        self.is_automatic() || matches!(self, Self::RequiredSubscribed)
    }
}

// Private: clients never subscribe to this table directly. `visible_subscriptions`
// is the way clients read subscription rows.
#[derive(Clone)]
#[spacetimedb::table(accessor = subscriptions)]
pub struct Subscription {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub subscriber_account_id: u64,
    #[index(btree)]
    pub subscriber_email: String,
    #[index(btree)]
    pub category_id: u64,
    pub subscribed_at: Timestamp,
    #[index(btree)]
    pub status: SubscriptionStatus,
}

// Private: clients never subscribe to this table directly. `active_unsubscribe_tokens`
// is the way clients read unsubscribe-token rows.
#[derive(Clone)]
#[spacetimedb::table(accessor = subscription_unsubscribe_tokens)]
pub struct SubscriptionUnsubscribeToken {
    #[primary_key]
    pub token: String,
    #[unique]
    pub subscription_id: u64,
    #[index(btree)]
    pub created_at: Timestamp,
    pub active: bool,
    pub revoked_at: Timestamp,
}
