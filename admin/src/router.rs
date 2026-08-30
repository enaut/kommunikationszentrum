use crate::module_bindings::dioxus::{
    use_connection_state, use_subscription, use_table_visible_admin_identities, ConnectionState,
};

#[derive(Clone, PartialEq)]
pub enum ActiveView {
    MySubscriptions,
    Messages,
    Categories,
    Members,
    ManagementConfiguration,
    ManagementStatus,
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
