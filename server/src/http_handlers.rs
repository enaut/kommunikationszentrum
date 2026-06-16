use crate::account::webhook_tokens;
use crate::account::UserSyncData;
use crate::mailing::{message_categories, unsubscribe_subscription_by_token};
use crate::mta::MtaConnectionLog;
use crate::mta::{blocked_ips, mta_connection_log};
use log::info;
use serde::Deserialize;
use serde_json::json;
use spacetimedb::{
    http::{Body, HandlerContext, Request as HttpRequest, Response as HttpResponse, Router},
    Table,
};
use stalwart_mta_hook_types::{
    Modification, Request as MtaHookRequest, Response as MtaHookResponse, Stage,
};

fn json_response(status: u16, value: serde_json::Value) -> HttpResponse {
    let body = serde_json::to_vec(&value).unwrap_or_default();
    HttpResponse::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from_bytes(body))
        .unwrap()
}

fn token_has_permission(ctx: &mut HandlerContext, token: &str, permission: &str) -> bool {
    info!("Check if token has permission");
    let hash = hex::encode(blake3::hash(token.as_bytes()).as_bytes());
    ctx.with_tx(|tx| {
        if let Some(t) = tx.db.webhook_tokens().token_hash().find(&hash) {
            let permission = t.active && t.permissions.iter().any(|p| p == permission);
            info!("Token has permission: {}", permission);
            permission
        } else {
            info!("Token not found");
            false
        }
    })
}

fn query_param_token(request: &HttpRequest) -> Option<String> {
    let query = request.uri().query()?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?.trim();
        let value = parts.next().unwrap_or_default().trim();
        if key == "token" && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

#[spacetimedb::http::handler]
fn mta_hook_handler(ctx: &mut HandlerContext, request: HttpRequest) -> HttpResponse {
    // Authentication
    let token = match request
        .headers()
        .get("authorization")
        .and_then(|hv| hv.to_str().ok())
        .and_then(|s| {
            s.strip_prefix("Bearer ")
                .or_else(|| s.strip_prefix("bearer "))
        })
        .map(|s| s.trim().to_string())
    {
        Some(t) => t,
        None => return json_response(401, json!({"error":"missing Authorization bearer token"})),
    };
    if !token_has_permission(ctx, &token, "mta-hook") {
        return json_response(403, json!({"error":"forbidden"}));
    }

    // Read body
    let body_bytes: Vec<u8> = request.into_body().into_bytes().into();
    let mta_req: MtaHookRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => {
            info!("Parsed MtaHookRequest: {:?}", r);
            r
        }
        Err(_) => {
            info!("Failed to parse MtaHookRequest");
            return json_response(400, json!({"error":"invalid JSON"}));
        }
    };

    match mta_req.context.stage {
        Stage::Data => {
            // persist message using the existing module routines in a transaction
            let _ = ctx.with_tx(|tx| {
                crate::mta::handle_data_stage(tx, &mta_req, tx.timestamp);
            });

            let resp =
                MtaHookResponse::accept().with_modifications(vec![Modification::add_header(
                    "X-Processed-By".to_string(),
                    "SpacetimeDB Kommunikationszentrum".to_string(),
                )]);

            let body = serde_json::to_vec(&resp).unwrap_or_default();
            HttpResponse::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(Body::from_bytes(body))
                .unwrap()
        }
        Stage::Connect => {
            let accepted = ctx.with_tx(|tx| {
                let client_ip = &mta_req.context.client.ip;
                if let Some(blocked) = tx.db.blocked_ips().ip().find(client_ip) {
                    if blocked.active {
                        tx.db.mta_connection_log().insert(MtaConnectionLog {
                            id: 0,
                            client_ip: "[REDACTED]".to_string(),
                            stage: "connect".to_string(),
                            action: "reject".to_string(),
                            timestamp: tx.timestamp,
                            details: "IP blocked".to_string(),
                        });
                        return false;
                    }
                }
                tx.db.mta_connection_log().insert(MtaConnectionLog {
                    id: 0,
                    client_ip: client_ip.to_string(),
                    stage: "connect".to_string(),
                    action: "accept".to_string(),
                    timestamp: tx.timestamp,
                    details: "Connection accepted".to_string(),
                });
                true
            });

            let resp = if accepted {
                MtaHookResponse::accept()
            } else {
                MtaHookResponse::reject(550, "IP blocked".to_string())
            };
            let body = serde_json::to_vec(&resp).unwrap_or_default();
            HttpResponse::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(Body::from_bytes(body))
                .unwrap()
        }
        Stage::Ehlo => {
            let accepted = ctx.with_tx(|tx| {
                if let Some(helo) = &mta_req.context.client.helo {
                    if helo.trim().is_empty() {
                        tx.db.mta_connection_log().insert(MtaConnectionLog {
                            id: 0,
                            client_ip: mta_req.context.client.ip.clone(),
                            stage: "ehlo".to_string(),
                            action: "reject".to_string(),
                            timestamp: tx.timestamp,
                            details: "Invalid EHLO/HELO: empty".to_string(),
                        });
                        return false;
                    }
                }
                tx.db.mta_connection_log().insert(MtaConnectionLog {
                    id: 0,
                    client_ip: mta_req.context.client.ip.clone(),
                    stage: "ehlo".to_string(),
                    action: "accept".to_string(),
                    timestamp: tx.timestamp,
                    details: "Valid EHLO".to_string(),
                });
                true
            });

            let resp = if accepted {
                MtaHookResponse::accept()
            } else {
                MtaHookResponse::reject(501, "Invalid EHLO/HELO argument".to_string())
            };
            let body = serde_json::to_vec(&resp).unwrap_or_default();
            HttpResponse::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(Body::from_bytes(body))
                .unwrap()
        }
        Stage::Mail => {
            let accepted = ctx.with_tx(|tx| {
                if let Some(envelope) = &mta_req.envelope {
                    let from_address = &envelope.from.address;
                    if !from_address.contains('@') || from_address.trim().is_empty() {
                        tx.db.mta_connection_log().insert(MtaConnectionLog {
                            id: 0,
                            client_ip: "[REDACTED]".to_string(),
                            stage: "mail".to_string(),
                            action: "reject".to_string(),
                            timestamp: tx.timestamp,
                            details: "Invalid sender address".to_string(),
                        });
                        return false;
                    }
                }
                tx.db.mta_connection_log().insert(MtaConnectionLog {
                    id: 0,
                    client_ip: mta_req.context.client.ip.clone(),
                    stage: "mail".to_string(),
                    action: "accept".to_string(),
                    timestamp: tx.timestamp,
                    details: "MAIL FROM accepted".to_string(),
                });
                true
            });

            let resp = if accepted {
                MtaHookResponse::accept()
            } else {
                MtaHookResponse::reject(550, "Invalid sender address".to_string())
            };
            let body = serde_json::to_vec(&resp).unwrap_or_default();
            HttpResponse::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(Body::from_bytes(body))
                .unwrap()
        }
        Stage::Rcpt => {
            let accepted = ctx.with_tx(|tx| {
                let mut accepted = false;
                if let Some(envelope) = &mta_req.envelope {
                    for recipient in &envelope.to {
                        let to_address = recipient.address.clone();
                        let category_found = tx
                            .db
                            .message_categories()
                            .email_address()
                            .find(&to_address)
                            .map_or(false, |c| c.active);
                        let action_str =
                            if category_found { "accept" } else { "reject" }.to_string();
                        tx.db.mta_connection_log().insert(MtaConnectionLog {
                            id: 0,
                            client_ip: "[REDACTED]".to_string(),
                            stage: "rcpt".to_string(),
                            action: action_str.clone(),
                            timestamp: tx.timestamp,
                            details: format!(
                                "Category validation: {}",
                                if category_found { "found" } else { "not found" }
                            ),
                        });
                        if category_found {
                            accepted = true;
                            break;
                        }
                    }
                }
                accepted
            });

            let resp = if accepted {
                MtaHookResponse::accept()
            } else {
                MtaHookResponse::reject(550, "No such mailing list".to_string())
            };

            let body = serde_json::to_vec(&resp).unwrap_or_default();
            HttpResponse::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(Body::from_bytes(body))
                .unwrap()
        }
        Stage::Auth => {
            ctx.with_tx(|tx| {
                tx.db.mta_connection_log().insert(MtaConnectionLog {
                    id: 0,
                    client_ip: mta_req.context.client.ip.clone(),
                    stage: "auth".to_string(),
                    action: "accept".to_string(),
                    timestamp: tx.timestamp,
                    details: "Auth stage - accept".to_string(),
                });
            });
            let resp = MtaHookResponse::accept();
            let body = serde_json::to_vec(&resp).unwrap_or_default();
            HttpResponse::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(Body::from_bytes(body))
                .unwrap()
        }
    }
}

#[derive(Deserialize)]
struct UserSyncPayload {
    action: String,
    user: UserSyncData,
}

#[spacetimedb::http::handler]
fn mailing_list_unsubscribe_handler(
    ctx: &mut HandlerContext,
    request: HttpRequest,
) -> HttpResponse {
    if request.method().as_str() != "POST" {
        return HttpResponse::builder()
            .status(405)
            .header("allow", "POST")
            .body(Body::from_bytes(b"method not allowed".to_vec()))
            .unwrap();
    }

    let token = match query_param_token(&request) {
        Some(token) => urlencoding::decode(&token).map(|s| s.into_owned()).unwrap_or(token),
        None => return json_response(400, json!({"error": "missing token query parameter"})),
    };

    let body_bytes: Vec<u8> = request.into_body().into_bytes().into();
    let body = String::from_utf8_lossy(&body_bytes).trim().to_string();
    if body != "List-Unsubscribe=One-Click" {
        return json_response(400, json!({"error": "invalid one-click payload"}));
    }

    let result: Result<(), String> =
        ctx.with_tx(|tx| unsubscribe_subscription_by_token(tx, token.clone()));
    match result {
        Ok(()) => json_response(200, json!({"status": "unsubscribed"})),
        Err(e) => {
            if e.contains("token") || e.contains("Subscription") {
                json_response(404, json!({"error": e}))
            } else {
                json_response(500, json!({"error": e}))
            }
        }
    }
}

#[spacetimedb::http::handler]
fn user_sync_handler(ctx: &mut HandlerContext, request: HttpRequest) -> HttpResponse {
    let token = match request
        .headers()
        .get("authorization")
        .and_then(|hv| hv.to_str().ok())
        .and_then(|s| {
            s.strip_prefix("Bearer ")
                .or_else(|| s.strip_prefix("bearer "))
        })
        .map(|s| s.trim().to_string())
    {
        Some(t) => t,
        None => return json_response(401, json!({"error":"missing Authorization bearer token"})),
    };
    if !token_has_permission(ctx, &token, "sync-user") {
        return json_response(403, json!({"error":"forbidden"}));
    }

    let body_bytes: Vec<u8> = request.into_body().into_bytes().into();
    let payload: UserSyncPayload = match serde_json::from_slice(&body_bytes) {
        Ok(p) => p,
        Err(_) => return json_response(400, json!({"error":"invalid JSON"})),
    };

    let user_data_str = match serde_json::to_string(&payload.user) {
        Ok(s) => s,
        Err(_) => return json_response(500, json!({"error":"serialization failed"})),
    };

    let result: Result<(), String> = ctx.with_tx(|tx| {
        crate::account::do_sync_user(tx, payload.action.clone(), user_data_str.clone())
    });

    match result {
        Ok(()) => json_response(
            200,
            json!({"status":"success","action":payload.action,"mitgliedsnr":payload.user.mitgliedsnr}),
        ),
        Err(e) => {
            if e.contains("Unauthorized") {
                json_response(403, json!({"error": e}))
            } else {
                json_response(500, json!({"error": e}))
            }
        }
    }
}

#[spacetimedb::http::router]
fn router() -> Router {
    Router::new()
        .post("/mta-hook", mta_hook_handler)
        .post("/user-sync", user_sync_handler)
        .post(
            "/mailing-list/unsubscribe",
            mailing_list_unsubscribe_handler,
        )
}
