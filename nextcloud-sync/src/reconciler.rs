use crate::config::Config;
use crate::models::map_ocs_user_to_upsert;
use crate::ocs_client::OcsClient;
use crate::worker::SyncJob;
use std::time::Duration;
use tokio::sync::mpsc;

pub async fn run_reconciler(config: Config, ocs_client: OcsClient, tx: mpsc::Sender<SyncJob>) {
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

                if users.is_empty() {
                    tracing::warn!(
                        "Received 0 users from Nextcloud OCS; aborting reconciliation to prevent accidental deactivations."
                    );
                    tokio::time::sleep(Duration::from_secs(interval_secs)).await;
                    continue;
                }

                let active_uids: Vec<String> = users
                    .iter()
                    .map(|u| u.id.clone())
                    .filter(|id| !id.trim().is_empty())
                    .collect();

                let mut synced = 0;
                let mut skipped = 0;
                let mut failed = 0;

                for ocs_user in users {
                    let uid = ocs_user.id.clone();
                    if let Some(req) = map_ocs_user_to_upsert(ocs_user) {
                        match tx.send(SyncJob::Upsert(req)).await {
                            Ok(_) => synced += 1,
                            Err(e) => {
                                tracing::error!(uid = %uid, error = %e, "Failed to enqueue user for reconciliation");
                                failed += 1;
                            }
                        }
                    } else {
                        skipped += 1;
                    }
                }

                if !active_uids.is_empty() {
                    if let Err(e) = tx.send(SyncJob::Reconcile(active_uids)).await {
                        tracing::error!(error = %e, "Failed to enqueue user reconciliation job");
                    }
                }

                tracing::info!(
                    total = total,
                    synced = synced,
                    skipped = skipped,
                    failed = failed,
                    "Completed Nextcloud full user reconciliation dispatch"
                );
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to list users from Nextcloud OCS during reconciliation");
            }
        }

        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}
