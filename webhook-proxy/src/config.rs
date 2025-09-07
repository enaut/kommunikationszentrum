use std::env;

/// Configuration for the webhook proxy server
#[derive(Debug, Clone)]
pub struct WebhookProxyConfig {
    /// SpacetimeDB server URI
    pub spacetimedb_uri: String,
    /// SpacetimeDB module name
    pub spacetimedb_module_name: String,
    /// Server bind address
    pub bind_address: String,
}

impl Default for WebhookProxyConfig {
    fn default() -> Self {
        Self {
            spacetimedb_uri: "http://localhost:3000".to_string(),
            spacetimedb_module_name: "kommunikation".to_string(),
            bind_address: "0.0.0.0:3002".to_string(),
        }
    }
}

impl WebhookProxyConfig {
    /// Load configuration from environment variables with defaults
    pub fn from_env() -> Self {
        Self {
            spacetimedb_uri: env::var("SPACETIMEDB_URI")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            spacetimedb_module_name: env::var("SPACETIMEDB_MODULE_NAME")
                .unwrap_or_else(|_| "kommunikation".to_string()),
            bind_address: env::var("WEBHOOK_PROXY_BIND_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:3002".to_string()),
        }
    }

    /// Load configuration from environment, attempting to load .env file first
    pub fn load() -> anyhow::Result<Self> {
        // Try to load .env file, but don't fail if it doesn't exist
        let _ = dotenvy::dotenv();

        Ok(Self::from_env())
    }
}