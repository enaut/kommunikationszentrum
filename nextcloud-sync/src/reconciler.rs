use crate::config::Config;
use crate::models::map_ocs_user_to_upsert;
use crate::ocs_client::OcsClient;
use crate::worker::SyncPoster;
use std::time::Duration;

pub async fn run_reconciler(config: Config, ocs_client: OcsClient, poster: SyncPoster) {
    let interval_secs = config.reconcile_interval_secs.max(60);
    tracing::info!(
        interval_secs = interval_secs,
        "Nextcloud reconciler started"
    );

    loop {
        tracing::info!("Starting Nextcloud full user reconciliation / backfill...");
        match ocs_client.list_all_users_details().await {
            Ok(users) => {
                let total = users.len();
                tracing::info!(count = total, "Fetched users from Nextcloud OCS");

                let mut synced = 0;
                let mut skipped = 0;
                let mut failed = 0;

                for ocs_user in users {
                    let uid = ocs_user.id.clone();
                    if let Some(req) = map_ocs_user_to_upsert(ocs_user) {
                        match poster.send_sync(&req).await {
                            Ok(_) => synced += 1,
                            Err(e) => {
                                tracing::error!(uid = %uid, error = %e, "Failed to reconcile user");
                                failed += 1;
                            }
                        }
                    } else {
                        skipped += 1;
                    }
                }

                tracing::info!(
                    total = total,
                    synced = synced,
                    skipped = skipped,
                    failed = failed,
                    "Completed Nextcloud full user reconciliation"
                );
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to list users from Nextcloud OCS during reconciliation");
            }
        }

        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}
