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
            .layer(CorsLayer::permissive())
            .with_state(self)
    }
    #[instrument(skip(self), fields(stage = ?request.context.stage))]
    async fn process_hook(&self, request: MtaHookRequest) -> Result<MtaHookResponse, String> {
        info!("Processing MTA hook request");

        // Convert the request to JSON and send to SpacetimeDB
        let hook_data = serde_json::to_string(&request).map_err(|e| {
            error!("Failed to serialize MTA hook request: {}", e);
            e.to_string()
        })?;

        debug!(
            "Calling SpacetimeDB reducer with data size: {} bytes",
            hook_data.len()
        );

        // Call SpacetimeDB reducer and wait for completion
        match self.db_connection.reducers.handle_mta_hook(hook_data) {
            Ok(_) => {
                debug!("Successfully called SpacetimeDB reducer");
            }
            Err(e) => {
                error!("Failed to call SpacetimeDB reducer: {}", e);
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
            Ok(resp) => info!(
                "MTA hook processed successfully with action: {:?}",
                resp.action
            ),
            Err(e) => error!("MTA hook processing failed: {}", e),
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
                "Processing RCPT TO stage for {} recipients",
                envelope.to.len()
            );

            for (index, recipient) in envelope.to.iter().enumerate() {
                let to_address = &recipient.address;
                debug!("Validating recipient {}/{}", index + 1, envelope.to.len());

                // For now, accept all recipients that look like email addresses
                // In a real implementation, you would check against SpacetimeDB categories
                if !to_address.contains('@') || to_address.trim().is_empty() {
                    warn!("Invalid recipient address format at index {}", index);
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
                "Processing DATA stage: {} bytes, {} recipients",
                message.size, to_count
            );

            // Extract subject from headers
            let subject = extract_subject_from_headers(&message.headers);
            debug!(
                "Message subject extracted (length: {} chars)",
                subject.len()
            );

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
                processing_time = processing_time,
                modifications_count = modifications.len(),
                "DATA processing completed - accepting with headers"
            );
            return Ok(MtaHookResponse::accept().with_modifications(modifications));
        }

        warn!("DATA accepted without envelope/message data");
        Ok(MtaHookResponse::accept())
    }

    // Test function to verify SpacetimeDB integration
    #[instrument(skip(self))]
    pub async fn test_spacetime_connection(&self) -> Result<(), String> {
        info!("Testing SpacetimeDB connection");

        // Test calling a simple reducer
        match self.db_connection.reducers.say_hello() {
            Ok(_) => {
                info!("SpacetimeDB connection test successful");
                Ok(())
            }
            Err(e) => {
                error!("SpacetimeDB connection test failed: {}", e);
                Err(format!("SpacetimeDB test failed: {}", e))
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

    match handler.process_hook(request).await {
        Ok(response) => {
            debug!(
                "MTA hook endpoint responding with action: {:?}",
                response.action
            );
            Ok(ResponseJson(response))
        }
        Err(e) => {
            error!("Error processing MTA hook in endpoint: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
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
            .with_module_name("kommunikationszentrum")
            .on_connect(|_, _, _| {
                info!("Connected to SpacetimeDB successfully");
            })
            .on_disconnect(|_, _| {
                warn!("Disconnected from SpacetimeDB");
            })
            .build()
            .map_err(|e| {
                error!("Failed to connect to SpacetimeDB: {}", e);
                std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e)
            })?,
    );

    info!("SpacetimeDB connection established successfully");

    // Run the connection loop in the background
    let connection_clone = db_connection.clone();
    tokio::spawn(async move {
        info!("Starting SpacetimeDB connection loop");
        if let Err(e) = connection_clone.run_async().await {
            error!("SpacetimeDB connection loop error: {}", e);
        }
    });

    // Give the connection some time to establish
    debug!("Waiting for connection to stabilize");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let mta_handler = Arc::new(MtaHookHandler::new(db_connection));

    // Test the SpacetimeDB connection
    if let Err(e) = mta_handler.test_spacetime_connection().await {
        warn!("SpacetimeDB test failed: {}", e);
        warn!("Continuing anyway, but data storage may not work");
    }

    let app = mta_handler.clone().router();

    info!("Binding TCP listener on 0.0.0.0:3002");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3002").await?;

    info!("MTA Hook server listening on http://localhost:3002/mta-hook");
    info!("Ready to receive Stalwart MTA hooks");

    axum::serve(listener, app).await?;

    Ok(())
}
