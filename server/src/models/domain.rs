use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[spacetimedb::table(accessor = domains)]
pub struct Domain {
    /// Stalwart domain ID (stable external identifier)
    #[primary_key]
    pub id: String,
    /// Domain name (e.g. "example.com")
    #[unique]
    pub name: String,
    /// Optional description from Stalwart
    pub description: Option<String>,
}
