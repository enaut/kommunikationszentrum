use crate::module_bindings::dioxus::{
    use_connection_state, use_table_admin_identities, ConnectionState,
};

#[derive(Clone, PartialEq)]
pub enum ActiveView {
    MySubscriptions,
    Categories,
    Members,
    Debug,
}

/// Returns `true` when the currently connected SpacetimeDB identity is present
/// in the `admin_identities` table.  Returns `false` while not yet connected.
pub fn use_is_admin() -> bool {
    let admin_identities = use_table_admin_identities();
    let state = use_connection_state();
    if let ConnectionState::Connected(identity, _) = state() {
        admin_identities().iter().any(|a| a.identity == identity)
    } else {
        false
    }
}
