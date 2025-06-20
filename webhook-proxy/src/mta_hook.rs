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
    async fn process_hook(&self, request: MtaHookRequest) -> Result<MtaHookResponse, String> {
        println!(
            "Processing MTA hook request for stage: {:?}",
            request.context.stage
        );

        // Convert the request to JSON and send to SpacetimeDB
        let hook_data = serde_json::to_string(&request).map_err(|e| e.to_string())?;

        println!(
            "Calling SpacetimeDB reducer with data: {} bytes",
            hook_data.len()
        );

        // Call SpacetimeDB reducer and wait for completion
        match self.db_connection.reducers.handle_mta_hook(hook_data) {
            Ok(_) => {
                println!("Successfully called SpacetimeDB reducer");
            }
            Err(e) => {
                eprintln!("Failed to call SpacetimeDB reducer: {}", e);
                return Err(format!("Failed to call SpacetimeDB reducer: {}", e));
            }
        }

        // Small delay to ensure reducer execution
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        match request.context.stage {
            Stage::Connect => self.handle_connect(&request).await,
            Stage::Ehlo => self.handle_ehlo(&request).await,
            Stage::Mail => self.handle_mail(&request).await,
            Stage::Rcpt => self.handle_rcpt(&request).await,
            Stage::Data => self.handle_data(&request).await,
            Stage::Auth => Ok(MtaHookResponse::accept()),
        }
    }

    async fn handle_connect(&self, request: &MtaHookRequest) -> Result<MtaHookResponse, String> {
        let _client_ip = &request.context.client.ip;
        println!("Connect stage - checking IP: [REDACTED]");

        // For now, just echo - accept all connections
        // In a real implementation, you would check against blocked IPs in SpacetimeDB
        println!("Connection accepted from IP: [REDACTED]");
        Ok(MtaHookResponse::accept())
    }

    async fn handle_ehlo(&self, request: &MtaHookRequest) -> Result<MtaHookResponse, String> {
        if let Some(helo) = &request.context.client.helo {
            println!("EHLO stage - HELO: [REDACTED]");

            // Basic validation - just check it's not empty
            if helo.trim().is_empty() {
                println!("Invalid EHLO/HELO: empty");
                return Ok(MtaHookResponse::reject(
                    501,
                    "Invalid EHLO/HELO argument".to_string(),
                ));
            }
        }

        println!("EHLO accepted");
        Ok(MtaHookResponse::accept())
    }

    async fn handle_mail(&self, request: &MtaHookRequest) -> Result<MtaHookResponse, String> {
        if let Some(envelope) = &request.envelope {
            let from_address = &envelope.from.address;
            println!("MAIL FROM stage - sender: [REDACTED]");

            // Basic email validation
            if !from_address.contains('@') || from_address.trim().is_empty() {
                println!("Invalid sender address");
                return Ok(MtaHookResponse::reject(
                    550,
                    "Invalid sender address".to_string(),
                ));
            }
        }

        println!("MAIL FROM accepted");
        Ok(MtaHookResponse::accept())
    }

    async fn handle_rcpt(&self, request: &MtaHookRequest) -> Result<MtaHookResponse, String> {
        if let Some(envelope) = &request.envelope {
            for recipient in &envelope.to {
                let to_address = &recipient.address;
                println!("RCPT TO stage - recipient: [REDACTED]");

                // For now, accept all recipients that look like email addresses
                // In a real implementation, you would check against SpacetimeDB categories
                if !to_address.contains('@') || to_address.trim().is_empty() {
                    println!("Invalid recipient address");
                    return Ok(MtaHookResponse::reject(
                        550,
                        "Invalid recipient address".to_string(),
                    ));
                }
            }
        }

        println!("RCPT TO accepted");
        Ok(MtaHookResponse::accept())
    }

    async fn handle_data(&self, request: &MtaHookRequest) -> Result<MtaHookResponse, String> {
        if let (Some(envelope), Some(message)) = (&request.envelope, &request.message) {
            let _from_address = &envelope.from.address;
            let to_addresses: Vec<String> = envelope.to.iter().map(|r| r.address.clone()).collect();

            println!(
                "DATA stage - from: [REDACTED], to: {:?}, size: {}",
                to_addresses.len(),
                message.size
            );

            // Extract subject from headers
            let _subject = extract_subject_from_headers(&message.headers);
            println!("Subject: [REDACTED]");

            // For now, just accept with processing headers
            let modifications = vec![
                Modification::add_header(
                    "X-Processed-By".to_string(),
                    "SpacetimeDB Kommunikationszentrum".to_string(),
                ),
                Modification::add_header(
                    "X-Processing-Time".to_string(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        .to_string(),
                ),
            ];

            println!("DATA processing completed - accepting with headers");
            return Ok(MtaHookResponse::accept().with_modifications(modifications));
        }

        println!("DATA accepted (no envelope/message)");
        Ok(MtaHookResponse::accept())
    }

    // Test function to verify SpacetimeDB integration
    pub async fn test_spacetime_connection(&self) -> Result<(), String> {
        println!("🧪 Testing SpacetimeDB connection...");

        // Test calling a simple reducer
        match self.db_connection.reducers.say_hello() {
            Ok(_) => {
                println!("✅ SpacetimeDB test successful");
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ SpacetimeDB test failed: {}", e);
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

async fn mta_hook_endpoint(
    State(handler): State<Arc<MtaHookHandler>>,
    Json(request): Json<MtaHookRequest>,
) -> Result<ResponseJson<MtaHookResponse>, StatusCode> {
    println!(
        "MTA Hook Request Received - Stage: {:?}",
        request.context.stage
    );

    match handler.process_hook(request).await {
        Ok(response) => {
            println!(
                "MTA hook processed successfully - Action: {:?}",
                response.action
            );
            Ok(ResponseJson(response))
        }
        Err(e) => {
            eprintln!("Error processing MTA hook: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Starting MTA Hook server...");

    // Connect to SpacetimeDB
    let db_connection = Arc::new(
        DbConnection::builder()
            .with_uri("http://localhost:3000")
            .with_module_name("kommunikationszentrum")
            .on_connect(|_, _, _| {
                println!("✅ Connected to SpacetimeDB successfully");
            })
            .on_disconnect(|_, _| {
                println!("❌ Disconnected from SpacetimeDB");
            })
            .build()
            .map_err(|e| {
                eprintln!("❌ Failed to connect to SpacetimeDB: {}", e);
                std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e)
            })?,
    );

    println!("🔗 Successfully established SpacetimeDB connection");

    // Run the connection loop in the background
    let connection_clone = db_connection.clone();
    tokio::spawn(async move {
        println!("🔄 Starting SpacetimeDB connection loop...");
        if let Err(e) = connection_clone.run_async().await {
            eprintln!("❌ SpacetimeDB connection loop error: {}", e);
        }
    });

    // Give the connection some time to establish
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let mta_handler = Arc::new(MtaHookHandler::new(db_connection));

    // Test the SpacetimeDB connection
    if let Err(e) = mta_handler.test_spacetime_connection().await {
        eprintln!("⚠️  Warning: SpacetimeDB test failed: {}", e);
        eprintln!("   Continuing anyway, but data storage may not work");
    }

    let app = mta_handler.clone().router();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3002").await?;

    println!("🚀 MTA Hook server listening on http://localhost:3002/mta-hook");
    println!("📧 Ready to receive Stalwart MTA hooks");

    axum::serve(listener, app).await?;

    Ok(())
}
