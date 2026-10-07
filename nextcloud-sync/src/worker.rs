use crate::config::Config;
use crate::models::UserSyncRequest;
use crate::ocs_client::OcsClient;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum SyncJob {
    SyncUser(String),   // uid to fetch and upsert
    DeleteUser(String), // uid to cascade delete
}

pub struct SyncPoster {
    client: reqwest::Client,
    sync_url: String,
    webhook_token: String,
}

impl SyncPoster {
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::builder().build().unwrap(),
            sync_url: config.spacetime_sync_url.clone(),
            webhook_token: config.spacetime_webhook_token.clone(),
        }
    }

    pub async fn send_sync(&self, request: &UserSyncRequest) -> Result<(), String> {
        let mut attempts = 0;
        let mut backoff = Duration::from_millis(500);

        loop {
            attempts += 1;
            let resp = self
                .client
                .post(&self.sync_url)
                .header("Authorization", format!("Bearer {}", self.webhook_token))
                .header("Content-Type", "application/json")
                .json(request)
                .send()
                .await;

            match resp {
                Ok(res) if res.status().is_success() => {
                    tracing::info!(
                        action = %request.action,
                        external_id = %request.user.external_id,
                        "Successfully posted user sync"
                    );
                    return Ok(());
                }
                Ok(res) if res.status().is_client_error() => {
                    let status = res.status();
                    let text = res.text().await.unwrap_or_default();
                    tracing::error!(
                        status = %status,
                        body = %text,
                        action = %request.action,
                        external_id = %request.user.external_id,
                        "Client error posting user sync; will not retry"
                    );
                    return Err(format!("Client error: {} - {}", status, text));
                }
                Ok(res) => {
                    let status = res.status();
                    let text = res.text().await.unwrap_or_default();
                    tracing::warn!(
                        attempt = attempts,
                        status = %status,
                        body = %text,
                        "Server error posting user sync; retrying..."
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        attempt = attempts,
                        error = %e,
                        "Network error posting user sync; retrying..."
                    );
                }
            }

            if attempts >= 5 {
                return Err(format!("Exceeded max retry attempts for user {}", request.user.external_id));
            }

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(10));
        }
    }
}

pub async fn run_sync_worker(
    mut rx: mpsc::Receiver<SyncJob>,
    ocs_client: OcsClient,
    poster: SyncPoster,
) {
    tracing::info!("Nextcloud user sync worker started");

    while let Some(job) = rx.recv().await {
        match job {
            SyncJob::SyncUser(uid) => {
                tracing::info!(uid = %uid, "Processing sync user job");
                match ocs_client.get_user(&uid).await {
                    Ok(Some(ocs_user)) => {
                        if let Some(req) = crate::models::map_ocs_user_to_upsert(ocs_user) {
                            if let Err(e) = poster.send_sync(&req).await {
                                tracing::error!(uid = %uid, error = %e, "Failed to upsert user");
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::warn!(uid = %uid, "User not found in Nextcloud; triggering delete");
                        let req = crate::models::map_uid_to_delete(&uid);
                        if let Err(e) = poster.send_sync(&req).await {
                            tracing::error!(uid = %uid, error = %e, "Failed to delete user");
                        }
                    }
                    Err(e) => {
                        tracing::error!(uid = %uid, error = %e, "Failed to fetch user from OCS");
                    }
                }
            }
            SyncJob::DeleteUser(uid) => {
                tracing::info!(uid = %uid, "Processing delete user job");
                let req = crate::models::map_uid_to_delete(&uid);
                if let Err(e) = poster.send_sync(&req).await {
                    tracing::error!(uid = %uid, error = %e, "Failed to delete user");
                }
            }
        }
    }

    tracing::warn!("Nextcloud user sync worker channel closed; worker exiting");
}
