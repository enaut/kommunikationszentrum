use axum::{
    extract::Json, http::StatusCode, response::Json as ResponseJson, routing::post, Router,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tower_http::cors::CorsLayer;

mod module_bindings;
use module_bindings::*;

#[derive(Debug, Deserialize)]
struct WebhookPayload {
    message: String,
    sender: String,
}

#[derive(Debug, Serialize)]
struct WebhookResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct SpacetimeWebhookPayload {
    message: String,
    sender: String,
    timestamp: u64,
}

async fn handle_webhook(
    Json(payload): Json<WebhookPayload>,
) -> Result<ResponseJson<WebhookResponse>, StatusCode> {
    println!("Received webhook payload: {:?}", payload);

    // Prepare the payload for SpacetimeDB
    let spacetime_payload = SpacetimeWebhookPayload {
        message: payload.message,
        sender: payload.sender,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    // Serialize the payload to JSON string
    let json_payload =
        serde_json::to_string(&spacetime_payload).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Connect to SpacetimeDB using generated bindings
    let connection = DbConnection::builder()
        .with_uri("http://localhost:3000")
        .with_module_name("kommunikationszentrum")
        .on_connect(|_, _, _| {
            println!("Connected to SpacetimeDB");
        })
        .on_disconnect(|_, _| {
            println!("Disconnected from SpacetimeDB");
        })
        .build()
        .map_err(|e| {
            eprintln!("Failed to connect to SpacetimeDB: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Call the handle_webhook reducer
    match connection.reducers.handle_webhook(json_payload) {
        Ok(_) => {
            println!("Successfully processed webhook");
            Ok(ResponseJson(WebhookResponse {
                success: true,
                message: "Webhook processed successfully".to_string(),
            }))
        }
        Err(e) => {
            eprintln!("Failed to call reducer: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Starting webhook proxy server on port 3001...");

    let app = Router::new()
        .route("/hook", post(handle_webhook))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;

    println!("Webhook proxy listening on http://localhost:3001/hook");
    println!("Send POST requests to http://localhost:3001/hook with JSON payload");

    axum::serve(listener, app).await?;

    Ok(())
}
