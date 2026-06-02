use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::post,
    Router,
};
use secrecy::ExposeSecret;
use spacetimedb_sdk::DbContext as _;
use stalwart_mta_hook_types::{
    Modification, Request as MtaHookRequest, Response as MtaHookResponse, Stage,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info, instrument, warn};
use tracing_subscriber::field::debug;

mod config;
mod module_bindings;

use config::WebhookProxyConfig;
use module_bindings::*;

#[derive(Clone)]
pub struct MtaHookHandler {
    db_connection: Arc<DbConnection>,
}

impl MtaHookHandler {
    pub fn new(db_connection: Arc<DbConnection>) -> Self {
        Self { db_connection }
    }

    pub fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/mta-hook", post(mta_hook_endpoint))
            .route("/user-sync", post(user_sync_endpoint))
            .layer(CorsLayer::permissive())
            .with_state(self)
    }
    #[instrument(skip(self), fields(stage = ?request.context.stage))]
    async fn process_hook(&self, request: MtaHookRequest) -> Result<MtaHookResponse, String> {
        info!("Processing MTA hook request");

        let hook_data = serde_json::to_string(&request).map_err(|e| {
            error!(error = %e, "Failed to serialize MTA hook request");
            e.to_string()
        })?;

        let response = match request.context.stage {
            // DATA stage: the reducer stores ReceivedMessage — wait for completion.
            Stage::Data => self.handle_data(&request, hook_data).await,
            // All other stages: dispatch the reducer for logging (fire-and-forget),
            // then decide the response using the local client-cache.
            ref stage => {
                if let Err(e) = self.db_connection.reducers.handle_mta_hook(hook_data) {
                    warn!(error = %e, "Failed to dispatch handle_mta_hook reducer");
                }
                match stage {
                    Stage::Connect => self.handle_connect(&request).await,
                    Stage::Ehlo => self.handle_ehlo(&request).await,
                    Stage::Mail => self.handle_mail(&request).await,
                    Stage::Rcpt => self.handle_rcpt(&request).await,
                    Stage::Auth => Ok(MtaHookResponse::accept()),
                    Stage::Data => unreachable!(),
                }
            }
        };

        match &response {
            Ok(resp) => info!(action = ?resp.action, "MTA hook processed successfully"),
            Err(e) => error!(error = %e, "MTA hook processing failed"),
        }

        response
    }

    #[instrument(skip(self, request), fields(client_ip = "[REDACTED]"))]
    async fn handle_connect(&self, request: &MtaHookRequest) -> Result<MtaHookResponse, String> {
        let _client_ip = &request.context.client.ip;
        debug!("Processing connect stage");

        // For now, just echo - accept all connections
        // In a real implementation, you would check against blocked IPs in SpacetimeDB
        info!("Connection accepted from client");
        Ok(MtaHookResponse::accept())
    }

    #[instrument(skip(self, request), fields(helo = "[REDACTED]"))]
    async fn handle_ehlo(&self, request: &MtaHookRequest) -> Result<MtaHookResponse, String> {
        if let Some(helo) = &request.context.client.helo {
            debug!("Processing EHLO stage");

            // Basic validation - just check it's not empty
            if helo.trim().is_empty() {
                warn!("Invalid EHLO/HELO: empty argument");
                return Ok(MtaHookResponse::reject(
                    501,
                    "Invalid EHLO/HELO argument".to_string(),
                ));
            }
        }

        info!("EHLO accepted");
        Ok(MtaHookResponse::accept())
    }

    #[instrument(skip(self, request), fields(sender = "[REDACTED]"))]
    async fn handle_mail(&self, request: &MtaHookRequest) -> Result<MtaHookResponse, String> {
        if let Some(envelope) = &request.envelope {
            let from_address = &envelope.from.address;
            debug!("Processing MAIL FROM stage");

            // Basic email validation
            if !from_address.contains('@') || from_address.trim().is_empty() {
                warn!("Invalid sender address format");
                return Ok(MtaHookResponse::reject(
                    550,
                    "Invalid sender address".to_string(),
                ));
            }
        }

        info!("MAIL FROM accepted");
        Ok(MtaHookResponse::accept())
    }

    #[instrument(skip(self, request), fields(recipients = "[REDACTED]"))]
    async fn handle_rcpt(&self, request: &MtaHookRequest) -> Result<MtaHookResponse, String> {
        let Some(envelope) = &request.envelope else {
            warn!("RCPT TO stage received without envelope");
            return Ok(MtaHookResponse::reject(550, "No envelope data".to_string()));
        };

        debug!("{:?},\n{}\n\n", envelope, "Processing RCPT TO stage");

        for (index, recipient) in envelope.to.iter().enumerate() {
            let to_address = &recipient.address;
            debug!(
                recipient_index = index + 1,
                total_recipients = envelope.to.len(),
                to_address,
                "Validating recipient against message categories"
            );

            // O(1) lookup via the unique index cached by the SpacetimeDB subscription.
            let category = self
                .db_connection
                .db
                .message_categories()
                .email_address()
                .find(to_address);

            debug!("{:?}", category);

            match category {
                Some(cat) if cat.active => {
                    info!(recipient = %to_address, "Recipient is a known active mailing list — accepting RCPT");
                    // Accept immediately as requested
                    return Ok(MtaHookResponse::accept());
                }
                Some(_) => {
                    warn!(
                        recipient_index = index,
                        "Recipient is an inactive mailing list"
                    );
                    // continue checking other recipients
                }
                None => {
                    warn!(
                        recipient_index = index,
                        "Recipient address does not match any message category"
                    );
                    // continue checking other recipients (likely a catchall/alias)
                }
            }
        }

        warn!("No recipients matched an active mailing list — rejecting RCPT TO");
        Ok(MtaHookResponse::reject(
            550,
            "No such mailing list".to_string(),
        ))
    }

    /// Call the `handle_mta_hook` reducer for the DATA stage and wait for it to complete.
    ///
    /// The reducer stores the message in `received_message` (when the sender is subscribed).
    /// We wait for the reducer so we only return "accept" once the message is persisted.
    #[instrument(skip(self, request, hook_data), fields(message_size = request.message.as_ref().map(|m| m.size)))]
    async fn handle_data(
        &self,
        request: &MtaHookRequest,
        hook_data: String,
    ) -> Result<MtaHookResponse, String> {
        let message_size = request.message.as_ref().map(|m| m.size).unwrap_or(0);
        let recipient_count = request.envelope.as_ref().map(|e| e.to.len()).unwrap_or(0);
        info!(
            message_size = message_size,
            recipient_count = recipient_count,
            "Processing DATA stage"
        );

        let (tx, rx) = tokio::sync::oneshot::channel();

        match self
            .db_connection
            .reducers
            .handle_mta_hook_then(hook_data, move |_ctx, result| {
                let _ = tx.send(result);
            }) {
            Ok(()) => {}
            Err(e) => {
                error!(error = %e, "Failed to dispatch handle_mta_hook reducer for DATA stage");
                return Err(format!("Failed to dispatch reducer: {}", e));
            }
        }

        match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
            Ok(Ok(Ok(Ok(())))) => {
                info!("Message processed by SpacetimeDB — accepting");
                Ok(
                    MtaHookResponse::accept().with_modifications(vec![Modification::add_header(
                        "X-Processed-By".to_string(),
                        "SpacetimeDB Kommunikationszentrum".to_string(),
                    )]),
                )
            }
            Ok(Ok(Ok(Err(reducer_err)))) => {
                error!(error = %reducer_err, "handle_mta_hook reducer returned error during DATA stage");
                Ok(MtaHookResponse::reject(
                    450,
                    "Temporary processing failure — please retry".to_string(),
                ))
            }
            Ok(Ok(Err(sdk_err))) => {
                error!(error = %sdk_err, "handle_mta_hook SDK error during DATA stage");
                Ok(MtaHookResponse::reject(
                    450,
                    "Temporary processing failure — please retry".to_string(),
                ))
            }
            Ok(Err(_)) => {
                error!("handle_mta_hook callback channel closed unexpectedly during DATA stage");
                Ok(MtaHookResponse::reject(
                    450,
                    "Temporary processing failure — please retry".to_string(),
                ))
            }
            Err(_elapsed) => {
                error!("handle_mta_hook reducer timed out after 5s during DATA stage");
                Ok(MtaHookResponse::reject(
                    450,
                    "Temporary processing failure — please retry".to_string(),
                ))
            }
        }
    }
}

fn extract_subject_from_headers(headers: &[(String, String)]) -> String {
    for (name, value) in headers {
        if name.to_lowercase() == "subject" {
            return value.trim().to_string();
        }
    }
    "No subject".to_string()
}

#[instrument(skip(handler, request), fields(stage = ?request.context.stage))]
async fn mta_hook_endpoint(
    State(handler): State<Arc<MtaHookHandler>>,
    Json(request): Json<MtaHookRequest>,
) -> Result<ResponseJson<MtaHookResponse>, StatusCode> {
    info!("MTA Hook request received");

    // Log the raw incoming payload for debugging recipient/catchall behavior.
    // This is a temporary diagnostic aid — remove or lower log level once investigated.
    match serde_json::to_string(&request) {
        Ok(raw) => debug!(payload = %raw, "Incoming raw MTA hook payload"),
        Err(e) => error!(error = %e, "Failed to serialize incoming MTA hook request for logging"),
    }

    match handler.process_hook(request).await {
        Ok(response) => {
            debug!(action = ?response.action, "MTA hook endpoint responding");
            Ok(ResponseJson(response))
        }
        Err(e) => {
            error!(error = %e, "Error processing MTA hook in endpoint");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[instrument(skip(handler, payload), fields(action = %payload.action, mitgliedsnr = %payload.user.mitgliedsnr))]
async fn user_sync_endpoint(
    State(handler): State<Arc<MtaHookHandler>>,
    Json(payload): Json<UserSyncPayload>,
) -> Result<ResponseJson<serde_json::Value>, (StatusCode, String)> {
    info!("Received user sync request:\n{:#?}", payload);

    let user_data = serde_json::to_string(&payload.user).map_err(|e| {
        error!(error = %e, "Failed to serialize user data");
        (StatusCode::BAD_REQUEST, e.to_string())
    })?;

    debug!(action = %payload.action, data_size = user_data.len(), "Calling sync_user reducer");

    // Use a oneshot channel so we can await the reducer's completion status.
    let (tx, rx) = tokio::sync::oneshot::channel();

    match handler.db_connection.reducers.sync_user_then(
        payload.action.clone(),
        user_data,
        move |_ctx, result| {
            let _ = tx.send(result);
        },
    ) {
        Ok(()) => {}
        Err(e) => {
            error!(error = %e, "Failed to dispatch sync_user reducer");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to dispatch sync_user: {}", e),
            ));
        }
    }

    // Wait up to 5 s for the reducer to complete.
    match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
        Ok(Ok(Ok(Ok(())))) => {
            info!(action = %payload.action, mitgliedsnr = %payload.user.mitgliedsnr, "User sync successful");
            Ok(ResponseJson(serde_json::json!({
                "status": "success",
                "action": payload.action,
                "mitgliedsnr": payload.user.mitgliedsnr
            })))
        }
        Ok(Ok(Ok(Err(reducer_err)))) => {
            let status = if reducer_err.contains("Unauthorized") {
                warn!(error = %reducer_err, action = %payload.action, mitgliedsnr = %payload.user.mitgliedsnr, "User sync rejected: unauthorized");
                StatusCode::FORBIDDEN
            } else {
                error!(error = %reducer_err, action = %payload.action, mitgliedsnr = %payload.user.mitgliedsnr, "User sync reducer error");
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Err((status, reducer_err))
        }
        Ok(Ok(Err(sdk_err))) => {
            error!(error = %sdk_err, "sync_user reducer internal SDK error");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Reducer SDK error: {}", sdk_err),
            ))
        }
        Ok(Err(_recv_err)) => {
            error!("sync_user callback channel closed unexpectedly");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Callback channel closed unexpectedly".to_string(),
            ))
        }
        Err(_elapsed) => {
            error!(action = %payload.action, mitgliedsnr = %payload.user.mitgliedsnr, "sync_user reducer timed out after 5s");
            Err((
                StatusCode::GATEWAY_TIMEOUT,
                "Reducer did not respond within 5 seconds".to_string(),
            ))
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mta_hook=debug,info".into()),
        )
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .pretty()
        .init();

    info!("Starting MTA Hook server");

    // Load configuration
    let config = WebhookProxyConfig::load()?;
    info!("Loaded configuration: {:?}", config);

    // Connect to SpacetimeDB
    info!("Establishing SpacetimeDB connection");
    let first_start = config.spacetimedb_token.is_none();
    let module_name = config.spacetimedb_module_name.clone();
    let db_connection = Arc::new(
        DbConnection::builder()
            .with_uri(&config.spacetimedb_uri)
            .with_database_name(&config.spacetimedb_module_name)
            .with_token(
                config
                    .spacetimedb_token
                    .map(|t| t.expose_secret().to_string()),
            )
            .on_connect(move |_, identity, token| {
                if first_start {
                    eprintln!(
                        "
╔══════════════════════════════════════════════════════════════════╗
║        WEBHOOK-PROXY: FIRST START — ACTION REQUIRED              ║
╚══════════════════════════════════════════════════════════════════╝
No SPACETIMEDB_TOKEN was set, so a fresh identity was issued.
This identity will change on every restart until you fix it.

Follow these two steps once, then restart the proxy:

1. Persist the token
  ──────────────────────────
  Create or edit  webhook-proxy/.env  and add:

    SPACETIMEDB_TOKEN={token}

2. Grant admin rights
  ───────────────────────────
  With spacetimedb running, call the register reducer once:

    spacetime call {module_name} register_admin_identity '\"{identity}\"'
══════════════════════════════════════════════════════════════════
"
                    );
                } else {
                    info!("Connected to SpacetimeDB as {}", identity);
                }
            })
            .on_disconnect(|_, _| {
                warn!("Disconnected from SpacetimeDB");
            })
            .build()
            .map_err(|e| {
                error!(error = %e, "Failed to connect to SpacetimeDB");
                std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e)
            })?,
    );

    info!("SpacetimeDB connection established successfully");

    // Run the connection loop in the background
    let connection_clone = db_connection.clone();
    tokio::spawn(async move {
        info!("Starting SpacetimeDB connection loop");
        if let Err(e) = connection_clone.run_async().await {
            error!(error = %e, "SpacetimeDB connection loop error");
        }
    });

    // Subscribe to message_categories so the RCPT handler can validate recipients
    // against the local client cache without a round-trip to SpacetimeDB.
    let (sub_ready_tx, sub_ready_rx) = tokio::sync::oneshot::channel::<()>();
    let sub_ready_tx = std::sync::Arc::new(std::sync::Mutex::new(Some(sub_ready_tx)));
    db_connection
        .subscription_builder()
        .on_applied(move |_ctx| {
            if let Some(tx) = sub_ready_tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
        })
        .on_error(|_ctx, err| {
            error!("message_categories subscription error: {err}");
        })
        .subscribe(["SELECT * FROM message_categories"]);

    info!("Waiting for message_categories subscription to populate...");
    match tokio::time::timeout(std::time::Duration::from_secs(10), sub_ready_rx).await {
        Ok(_) => info!("message_categories subscription is ready"),
        Err(_) => warn!("message_categories subscription timed out after 10s — starting anyway"),
    }

    let mta_handler = Arc::new(MtaHookHandler::new(db_connection));

    let app = mta_handler.clone().router();

    info!(bind_address = %config.bind_address, "Binding TCP listener");
    let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;

    let server_url = format!(
        "http://{}/mta-hook",
        config.bind_address.replace("0.0.0.0", "localhost")
    );

    info!(
        server_url = %server_url,
        "MTA Hook server listening"
    );
    info!("Ready to receive Stalwart MTA hooks");

    axum::serve(listener, app).await?;

    Ok(())
}

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct UserSyncPayload {
    pub action: String, // "upsert" or "delete"
    pub user: UserSyncData,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UserSyncData {
    pub mitgliedsnr: u64,
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<bool>,
    pub is_admin: Option<bool>,
    pub updated_at: Option<String>,
}
