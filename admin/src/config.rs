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
        Self {
            issuer_url: "http://127.0.0.1:8000/o".to_string(),
            client_id: "admin-app".to_string(),
            redirect_uri: "http://127.0.0.1:8080/callback".to_string(),
            scope: "openid profile email".to_string(),
        }
    }
}

impl AdminConfig {
    /// Load configuration from build-time environment variables with defaults
    pub fn from_env() -> Self {
        Self {
            spacetimedb_uri: option_env!("SPACETIMEDB_URI")
                .unwrap_or("http://localhost:3000")
                .to_string(),
            spacetimedb_module_name: option_env!("SPACETIMEDB_MODULE_NAME")
                .unwrap_or("kommunikation")
                .to_string(),
            oauth: OAuthConfig {
                issuer_url: option_env!("OIDC_ISSUER_URL")
                    .unwrap_or("http://127.0.0.1:8000/o")
                    .to_string(),
                client_id: option_env!("OIDC_CLIENT_ID")
                    .unwrap_or("admin-app")
                    .to_string(),
                redirect_uri: option_env!("ADMIN_REDIRECT_URI")
                    .unwrap_or("http://127.0.0.1:8080/callback")
                    .to_string(),
                scope: option_env!("OAUTH_SCOPES")
                    .unwrap_or("openid profile email")
                    .to_string(),
            },
        }
    }

    /// Load configuration
    pub fn load() -> Self {
        Self::from_env()
    }
}
