use crate::config::Config;
use crate::worker::SyncJob;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use subtle::ConstantTimeEq;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub tx: mpsc::Sender<SyncJob>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookPayload {
    /// Nextcloud event class or identifier
    #[serde(default)]
    pub event: Option<String>,
    /// User identifier if directly at top level or nested
    #[serde(default)]
    pub user: Option<WebhookUserTarget>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub uid: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookUserTarget {
    #[serde(default)]
    pub uid: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/nextcloud/webhook", post(handle_webhook))
        .route("/health", axum::routing::get(handle_health))
        .with_state(state)
}

async fn handle_health() -> impl IntoResponse {
    StatusCode::OK
}

async fn handle_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    // Authenticate webhook request
    let expected_secret = &state.config.webhook_secret;
    let auth_header = headers
        .get("X-Nextcloud-Token")
        .or_else(|| headers.get("X-Webhook-Secret"))
        .or_else(|| headers.get("Authorization"))
        .and_then(|val| val.to_str().ok())
        .map(|s| s.strip_prefix("Bearer ").unwrap_or(s).trim());

    let is_valid = match auth_header {
        Some(token) => {
            let a = token.as_bytes();
            let b = expected_secret.as_bytes();
            if a.len() == b.len() {
                a.ct_eq(b).into()
            } else {
                false
            }
        }
        None => false,
    };

    if !is_valid {
        tracing::warn!("Rejected webhook with missing or invalid secret");
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    let uid = payload
        .uid
        .as_deref()
        .or(payload.target.as_deref())
        .or_else(|| {
            payload
                .user
                .as_ref()
                .and_then(|u| u.uid.as_deref().or(u.id.as_deref()))
        })
        .map(|s| s.to_string());

    let uid = match uid {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => {
            tracing::warn!(payload = ?payload, "Webhook received without extractable user id");
            return (StatusCode::BAD_REQUEST, "Missing user ID").into_response();
        }
    };

    let event_name = payload.event.as_deref().unwrap_or("UserChangedEvent");
    let is_delete = event_name.contains("UserDeletedEvent") || event_name.contains("delete");

    let job = if is_delete {
        tracing::info!(uid = %uid, event = %event_name, "Queueing user delete from webhook");
        SyncJob::DeleteUser(uid)
    } else {
        tracing::info!(uid = %uid, event = %event_name, "Queueing user sync from webhook");
        SyncJob::SyncUser(uid)
    };

    if let Err(e) = state.tx.send(job).await {
        tracing::error!(error = %e, "Failed to send job to sync worker channel");
        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal channel error").into_response();
    }

    (StatusCode::ACCEPTED, "Accepted").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_webhook_payload() {
        let json = r#"{
            "event": "OCP\\User\\Events\\UserCreatedEvent",
            "user": { "uid": "john_doe" }
        }"#;
        let p: WebhookPayload = serde_json::from_str(json).unwrap();
        assert_eq!(p.event.as_deref(), Some("OCP\\User\\Events\\UserCreatedEvent"));
        assert_eq!(p.user.unwrap().uid.as_deref(), Some("john_doe"));

        let json_target = r#"{
            "event": "OCP\\User\\Events\\UserDeletedEvent",
            "target": "john_doe"
        }"#;
        let p2: WebhookPayload = serde_json::from_str(json_target).unwrap();
        assert_eq!(p2.target.as_deref(), Some("john_doe"));
    }
}
