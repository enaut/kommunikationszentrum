use std::env;

/// Configuration for the admin web application
#[derive(Debug, Clone)]
pub struct AdminConfig {
    /// SpacetimeDB server URI
    pub spacetimedb_uri: String,
    /// SpacetimeDB module name
    pub spacetimedb_module_name: String,
    /// OAuth configuration
    pub oauth: OAuthConfig,
}

/// OAuth/OIDC configuration
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// OIDC issuer URL (discovery endpoint base)
    pub issuer_url: String,
    /// OAuth client ID
    pub client_id: String,
    /// OAuth redirect URI
    pub redirect_uri: String,
    /// OAuth scopes (space-separated)
    pub scope: String,
    /// Django base URL (for backward compatibility)
    pub django_base_url: String,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            spacetimedb_uri: "http://localhost:3000".to_string(),
            spacetimedb_module_name: "kommunikation".to_string(),
            oauth: OAuthConfig::default(),
        }
    }
}

impl Default for OAuthConfig {
    fn default() -> Self {
        let django = "http://127.0.0.1:8000".to_string();
        Self {
            issuer_url: format!("{django}/o"),
            client_id: "admin-app".to_string(),
            redirect_uri: "http://127.0.0.1:8080/callback".to_string(),
            scope: "openid profile email".to_string(),
            django_base_url: django,
        }
    }
}

impl AdminConfig {
    /// Load configuration from environment variables with defaults
    pub fn from_env() -> Self {
        let django_base_url = env::var("DJANGO_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());

        Self {
            spacetimedb_uri: env::var("SPACETIMEDB_URI")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            spacetimedb_module_name: env::var("SPACETIMEDB_MODULE_NAME")
                .unwrap_or_else(|_| "kommunikation".to_string()),
            oauth: OAuthConfig {
                issuer_url: env::var("OIDC_ISSUER_URL")
                    .unwrap_or_else(|_| format!("{django_base_url}/o")),
                client_id: env::var("OIDC_CLIENT_ID")
                    .unwrap_or_else(|_| "admin-app".to_string()),
                redirect_uri: env::var("ADMIN_REDIRECT_URI")
                    .unwrap_or_else(|_| "http://127.0.0.1:8080/callback".to_string()),
                scope: env::var("OAUTH_SCOPES")
                    .unwrap_or_else(|_| "openid profile email".to_string()),
                django_base_url,
            },
        }
    }

    /// Load configuration from environment, attempting to load .env file first
    pub fn load() -> Self {
        // Try to load .env file, but don't fail if it doesn't exist
        // For WASM targets, this will be a no-op
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = dotenvy::dotenv();
        }

        Self::from_env()
    }
}