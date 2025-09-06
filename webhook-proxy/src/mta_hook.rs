use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::post,
    Router,
};
use stalwart_mta_hook_types::{
    Modification, Request as MtaHookRequest, Response as MtaHookResponse, Stage,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info, instrument, warn};

mod module_bindings;
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

        // Convert the request to JSON and send to SpacetimeDB
        let hook_data = serde_json::to_string(&request).map_err(|e| {
            error!(error = %e, "Failed to serialize MTA hook request");
            e.to_string()
        })?;

        debug!(data_size = hook_data.len(), "Calling SpacetimeDB reducer");

        // Call SpacetimeDB reducer and wait for completion
        match self.db_connection.reducers.handle_mta_hook(hook_data) {
            Ok(_) => {
                debug!("Successfully called SpacetimeDB reducer");
            }
            Err(e) => {
                error!(error = %e, "Failed to call SpacetimeDB reducer");
                return Err(format!("Failed to call SpacetimeDB reducer: {}", e));
            }
        }

        // Small delay to ensure reducer execution
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let response = match request.context.stage {
            Stage::Connect => self.handle_connect(&request).await,
            Stage::Ehlo => self.handle_ehlo(&request).await,
            Stage::Mail => self.handle_mail(&request).await,
            Stage::Rcpt => self.handle_rcpt(&request).await,
            Stage::Data => self.handle_data(&request).await,
            Stage::Auth => Ok(MtaHookResponse::accept()),
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
        if let Some(envelope) = &request.envelope {
            debug!(
                recipient_count = envelope.to.len(),
                "Processing RCPT TO stage"
            );

            for (index, recipient) in envelope.to.iter().enumerate() {
                let to_address = &recipient.address;
                debug!(
                    recipient_index = index + 1,
                    total_recipients = envelope.to.len(),
                    "Validating recipient"
                );

                // For now, accept all recipients that look like email addresses
                // In a real implementation, you would check against SpacetimeDB categories
                if !to_address.contains('@') || to_address.trim().is_empty() {
                    warn!(recipient_index = index, "Invalid recipient address format");
                    return Ok(MtaHookResponse::reject(
                        550,
                        "Invalid recipient address".to_string(),
                    ));
                }
            }
        }

        info!("RCPT TO accepted");
        Ok(MtaHookResponse::accept())
    }

    #[instrument(skip(self, request), fields(message_size = request.message.as_ref().map(|m| m.size)))]
    async fn handle_data(&self, request: &MtaHookRequest) -> Result<MtaHookResponse, String> {
        if let (Some(envelope), Some(message)) = (&request.envelope, &request.message) {
            let _from_address = &envelope.from.address;
            let to_count = envelope.to.len();

            info!(
                message_size = message.size,
                recipient_count = to_count,
                "Processing DATA stage"
            );

            // Extract subject from headers
            let subject = extract_subject_from_headers(&message.headers);
            debug!(subject_length = subject.len(), "Message subject extracted");

            // Log message processing details
            debug!(
                message_size = message.size,
                recipient_count = to_count,
                header_count = message.headers.len(),
                "Message metadata"
            );

            // For now, just accept with processing headers
            let processing_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_string();

            let modifications = vec![
                Modification::add_header(
                    "X-Processed-By".to_string(),
                    "SpacetimeDB Kommunikationszentrum".to_string(),
                ),
                Modification::add_header("X-Processing-Time".to_string(), processing_time.clone()),
            ];

            info!(
                processing_time = %processing_time,
                modifications_count = modifications.len(),
                "DATA processing completed - accepting with headers"
            );
            return Ok(MtaHookResponse::accept().with_modifications(modifications));
        }

        warn!("DATA accepted without envelope/message data");
        Ok(MtaHookResponse::accept())
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

    // Call SpacetimeDB sync_user reducer
    match handler
        .db_connection
        .reducers
        .sync_user(payload.action.clone(), user_data)
    {
        Ok(()) => {
            info!(action = %payload.action, mitgliedsnr = %payload.user.mitgliedsnr, "User sync successful");
            Ok(ResponseJson(serde_json::json!({
                "status": "success",
                "action": payload.action,
                "mitgliedsnr": payload.user.mitgliedsnr
            })))
        }
        Err(e) => {
            error!(error = %e, action = %payload.action, mitgliedsnr = %payload.user.mitgliedsnr, "User sync failed");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to sync user: {}", e),
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
        .init();

    info!("Starting MTA Hook server");

    // Connect to SpacetimeDB
    info!("Establishing SpacetimeDB connection");
    let db_connection = Arc::new(
        DbConnection::builder()
            .with_uri("http://localhost:3000")
            .with_module_name("kommunikation")
            .on_connect(|_, _, _| {
                info!("Connected to SpacetimeDB successfully");
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

    let mta_handler = Arc::new(MtaHookHandler::new(db_connection));

    let app = mta_handler.clone().router();

    info!(bind_address = "0.0.0.0:3002", "Binding TCP listener");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3002").await?;

    info!(
        server_url = "http://localhost:3002/mta-hook",
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
    pub updated_at: Option<String>,
    pub identity_hex: Option<String>,
}
