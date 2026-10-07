mod config;
mod models;
mod ocs_client;
mod reconciler;
mod webhook;
mod worker;

use config::Config;
use ocs_client::OcsClient;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use worker::{run_sync_worker, SyncPoster};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nextcloud_sync=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Nextcloud -> Kommunikationszentrum sync service");

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Configuration error: {}", e);
            std::process::exit(1);
        }
    };

    let ocs_client = OcsClient::new(&config);

    // Bounded in-memory channel for sync jobs
    let (tx, rx) = mpsc::channel(1000);

    // Spawn async background worker
    let worker_ocs = ocs_client.clone();
    let worker_poster = SyncPoster::new(&config);
    tokio::spawn(async move {
        run_sync_worker(rx, worker_ocs, worker_poster).await;
    });

    // Spawn async reconciler (initial backfill + periodic sweep)
    let reconciler_config = config.clone();
    let reconciler_ocs = ocs_client.clone();
    let reconciler_poster = SyncPoster::new(&config);
    tokio::spawn(async move {
        reconciler::run_reconciler(reconciler_config, reconciler_ocs, reconciler_poster).await;
    });

    // Start HTTP Webhook server
    let app_state = webhook::AppState {
        config: config.clone(),
        tx,
    };
    let app = webhook::router(app_state);

    let addr: SocketAddr = config.listen_addr.parse().map_err(|e| {
        format!(
            "Failed to parse listen address '{}': {}",
            config.listen_addr, e
        )
    })?;

    tracing::info!(addr = %addr, "Listening for Nextcloud webhooks");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
