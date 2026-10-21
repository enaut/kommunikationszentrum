use crate::module_bindings::SubscriptionPermission;
use crate::module_bindings::dioxus::{
    ConnectionState, use_connection_state, use_subscription, use_table_context_accounts,
    use_table_visible_admin_identities, use_table_visible_subscriptions,
};
use crate::pages::is_active_subscription;

#[derive(Clone, PartialEq)]
pub enum ActiveView {
    MySubscriptions,
    Messages,
    Topics,
    Members,
    ManagementConfiguration,
    ManagementStatus,
    Profile,
}

/// Returns `true` when the currently connected SpacetimeDB identity is present
/// in the `admin_identities` table.  Returns `false` while not yet connected.
pub fn use_is_admin() -> bool {
    // The navbar needs this view before it can decide whether to show admin-only links.
    use_subscription(&["SELECT * FROM visible_admin_identities"]);
    let admin_identities = use_table_visible_admin_identities();
    let state = use_connection_state();
    if let ConnectionState::Connected(identity, _) = state() {
        let res = admin_identities().iter().any(|a| a.identity == identity);
        res
    } else {
        false
    }
}

/// Returns the topic IDs for which the currently connected user has an active
/// subscription with `Moderate` permissions.
pub fn use_moderated_topic_ids() -> Vec<u64> {
    use_subscription(&[
        "SELECT * FROM context_accounts",
        "SELECT * FROM visible_subscriptions",
    ]);
    let state = use_connection_state();
    if let ConnectionState::Connected(identity, _) = state() {
        let accounts = use_table_context_accounts();
        let my_account_id = accounts()
            .iter()
            .find(|a| a.identity == identity)
            .map(|a| a.id);
        if let Some(acc_id) = my_account_id {
            let subscriptions = use_table_visible_subscriptions();
            return subscriptions()
                .iter()
                .filter(|s| {
                    s.subscriber_account_id == acc_id
                        && is_active_subscription(&s.status)
                        && s.permission == SubscriptionPermission::Moderate
                })
                .map(|s| s.topic_id)
                .collect();
        }
    }
    vec![]
}

/// Returns `true` if the connected user is a global Admin or has moderation rights
/// on at least one topic.
pub fn use_can_manage_topics() -> bool {
    let is_admin = use_is_admin();
    let moderated_topics = use_moderated_topic_ids();
    is_admin || !moderated_topics.is_empty()
}
