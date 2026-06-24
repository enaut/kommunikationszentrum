use std::{env, time::Duration};

#[derive(Debug, Clone)]
pub struct SenderConfig {
    pub spacetimedb_uri: String,
    pub spacetimedb_database_name: String,
    pub spacetimedb_token: Option<String>,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_use_tls: bool,
    pub poll_interval: Duration,
    pub message_id_domain: String,
    pub unsubscribe_base_url: String,
    pub otlp_endpoint: String,
}

impl SenderConfig {
    pub fn from_env() -> Self {
        let spacetimedb_uri =
            env::var("SPACETIMEDB_URI").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
        let spacetimedb_database_name =
            env::var("SPACETIMEDB_DATABASE_NAME").unwrap_or_else(|_| "kommunikation".to_string());
        let otlp_endpoint =
            env::var("OTLP_ENDPOINT").unwrap_or_else(|_| "http://localhost:4317".to_string());
        let smtp_host = env::var("SMTP_HOST").unwrap_or_else(|_| "mail-eu.smtp2go.com".to_string());
        let smtp_port = env::var("SMTP_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(8465);
        let smtp_username = env::var("SMTP_USERNAME").ok();
        let smtp_password = env::var("SMTP_PASSWORD").ok();
        let smtp_use_tls = env::var("SMTP_USE_TLS")
            .ok()
            .and_then(|value| value.parse::<bool>().ok())
            .unwrap_or(true);
        let poll_interval = env::var("SENDER_POLL_INTERVAL_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or_else(|| Duration::from_millis(5000));
        let message_id_domain = env::var("MAIL_MESSAGE_ID_DOMAIN").unwrap_or_else(|_| {
            spacetimedb_uri
                .split_once("//")
                .map(|(_, rest)| rest.split('/').next().unwrap_or("solawis.de").to_string())
                .unwrap_or_else(|| "solawis.de".to_string())
        });
        let unsubscribe_base_url = env::var("MAIL_UNSUBSCRIBE_BASE_URL").unwrap_or_else(|_| {
            format!(
                "{}/v1/database/{}/route/mailing-list/unsubscribe",
                spacetimedb_uri.trim_end_matches('/'),
                spacetimedb_database_name
            )
        });

        Self {
            spacetimedb_uri,
            spacetimedb_database_name,
            spacetimedb_token: env::var("SPACETIMEDB_TOKEN").ok(),
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password,
            smtp_use_tls,
            poll_interval,
            message_id_domain,
            unsubscribe_base_url,
            otlp_endpoint,
        }
    }
}
