use spacetimedb::{SpacetimeType, Timestamp};

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

impl SubscriptionPermission {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            _ => Err(format!(
                "Invalid subscription permission '{value}'; expected 'read' or 'write'"
            )),
        }
    }
}

impl TopicVisibility {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "public" | "Public" => Ok(Self::Public),
            "private" | "Private" => Ok(Self::Private),
            _ => Err(format!(
                "Invalid topic visibility '{value}'; expected 'public' or 'private'"
            )),
        }
    }
}

// Private: clients never subscribe to this table directly. `visible_message_topics`
// is the way clients read topic rows.
#[derive(Clone)]
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
    /// Controls whether regular members can see this topic. Admins can see both variants.
    #[index(btree)]
    #[default(TopicVisibility::Public)]
    pub visibility: TopicVisibility,
    /// Stalwart app password for SMTP submission from this topic mailbox.
    /// Set during `provision_message_topic`; absent for DB-only topics.
    #[index(btree)]
    #[default(None::<u64>)]
    pub app_password_id: Option<u64>,
    #[default(SubscriptionPermission::Read)]
    pub default_permission: SubscriptionPermission,
    #[default(false)]
    pub locked_is_provisioning: bool,
}

// Private: clients never subscribe to this table directly. `visible_topic_app_passwords`
// is the way clients read app-password rows.
#[spacetimedb::table(accessor = topic_app_passwords)]
pub struct TopicAppPassword {
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

/// Topic data as sent by the Django user-sync webhook for a single
/// mailing-list assignment (e.g. a Verteilpunkt).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TopicSyncData {
    pub name: String,
    pub email_address: String,
    pub description: String,
    #[serde(default = "default_topic_visibility")]
    pub visibility: String,
    /// `None` leaves an existing topic's categories unchanged. An empty array
    /// explicitly removes all of its category assignments.
    #[serde(default, alias = "topics")]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default_permission: Option<String>,
}

pub fn default_topic_visibility() -> String {
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
    pub account_email_id: u64,
    #[index(btree)]
    pub topic_id: u64,
    pub subscribed_at: Timestamp,
    #[index(btree)]
    pub status: SubscriptionStatus,
    pub permission: SubscriptionPermission,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_permission_parse() {
        assert_eq!(
            SubscriptionPermission::parse("read").unwrap(),
            SubscriptionPermission::Read
        );
        assert_eq!(
            SubscriptionPermission::parse("Read").unwrap(),
            SubscriptionPermission::Read
        );
        assert_eq!(
            SubscriptionPermission::parse("write").unwrap(),
            SubscriptionPermission::Write
        );
        assert_eq!(
            SubscriptionPermission::parse("WRITE").unwrap(),
            SubscriptionPermission::Write
        );
        assert!(SubscriptionPermission::parse("invalid").is_err());
        assert!(SubscriptionPermission::parse("writ").is_err());
        assert!(SubscriptionPermission::parse("").is_err());
    }

    #[test]
    fn test_topic_visibility_parse() {
        assert_eq!(
            TopicVisibility::parse("public").unwrap(),
            TopicVisibility::Public
        );
        assert_eq!(
            TopicVisibility::parse("Public").unwrap(),
            TopicVisibility::Public
        );
        assert_eq!(
            TopicVisibility::parse("private").unwrap(),
            TopicVisibility::Private
        );
        assert_eq!(
            TopicVisibility::parse("Private").unwrap(),
            TopicVisibility::Private
        );
        assert!(TopicVisibility::parse("publc").is_err());
        assert!(TopicVisibility::parse("invalid").is_err());
        assert!(TopicVisibility::parse("").is_err());
    }

    #[test]
    fn test_topic_sync_data_deserialization() {
        let json_str = r#"{
            "name": "VP Nord",
            "email_address": "vp-nord@solawi.org",
            "description": "Verteilpunkt Nord",
            "categories": ["Verteilpunkt"],
            "required": true,
            "default_permission": "write"
        }"#;

        let data: TopicSyncData = serde_json::from_str(json_str).unwrap();
        assert_eq!(data.name, "VP Nord");
        assert_eq!(data.email_address, "vp-nord@solawi.org");
        assert_eq!(data.categories, Some(vec!["Verteilpunkt".to_string()]));
        assert!(data.required);
        assert_eq!(data.default_permission.as_deref(), Some("write"));
        assert_eq!(
            SubscriptionPermission::parse(data.default_permission.as_deref().unwrap()).unwrap(),
            SubscriptionPermission::Write
        );
    }

    #[test]
    fn test_topic_sync_data_invalid_permission_detected() {
        let json_str = r#"{
            "name": "VP Nord",
            "email_address": "vp-nord@solawi.org",
            "description": "Verteilpunkt Nord",
            "default_permission": "writ"
        }"#;

        let data: TopicSyncData = serde_json::from_str(json_str).unwrap();
        assert_eq!(data.default_permission.as_deref(), Some("writ"));
        assert!(SubscriptionPermission::parse(data.default_permission.as_deref().unwrap()).is_err());
    }

    #[test]
    fn test_topic_sync_data_topics_alias() {
        let json_str = r#"{
            "name": "VP Nord",
            "email_address": "vp-nord@solawi.org",
            "description": "Verteilpunkt Nord",
            "topics": ["Verteilpunkt"]
        }"#;

        let data: TopicSyncData = serde_json::from_str(json_str).unwrap();
        assert_eq!(data.categories, Some(vec!["Verteilpunkt".to_string()]));
    }
}
