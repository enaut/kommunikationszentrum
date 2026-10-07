use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub listen_addr: String,
    pub webhook_secret: Option<String>,
    pub nc_url: String,
    pub nc_user: String,
    pub nc_app_password: String,
    pub reconcile_interval_secs: u64,
    pub spacetime_sync_url: String,
    pub spacetime_webhook_token: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let listen_addr = env::var("NC_SYNC_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8088".to_string());
        let webhook_secret = env::var("NC_WEBHOOK_SECRET").ok().filter(|s| !s.trim().is_empty());
        let nc_url = env::var("NC_URL").map_err(|_| "NC_URL environment variable is required".to_string())?;
        let nc_user = env::var("NC_USER").map_err(|_| "NC_USER environment variable is required".to_string())?;
        let nc_app_password = env::var("NC_APP_PASSWORD").map_err(|_| "NC_APP_PASSWORD environment variable is required".to_string())?;
        
        let reconcile_interval_secs = env::var("NC_RECONCILE_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(21600); // 6 hours default

        let spacetime_sync_url = env::var("SPACETIME_SYNC_URL")
            .map_err(|_| "SPACETIME_SYNC_URL environment variable is required".to_string())?;
        let spacetime_webhook_token = env::var("SPACETIME_WEBHOOK_TOKEN")
            .map_err(|_| "SPACETIME_WEBHOOK_TOKEN environment variable is required".to_string())?;

        Ok(Self {
            listen_addr,
            webhook_secret,
            nc_url: nc_url.trim_end_matches('/').to_string(),
            nc_user,
            nc_app_password,
            reconcile_interval_secs,
            spacetime_sync_url,
            spacetime_webhook_token,
        })
    }
}
