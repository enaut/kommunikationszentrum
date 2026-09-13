use crate::models::account::webhook_tokens;
use crate::models::mta::{blocked_ips, mta_connection_log, MtaConnectionLog};
use crate::models::topic::{
    message_topics, MessageTopic, SubscriptionPermission, TopicSyncData, TopicVisibility,
};
use crate::reducers::{do_sync_user, unsubscribe_subscription_by_token, UserSyncData};
use crate::services::stalwart::topic::provision_stalwart_topic_mailbox;
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

#[spacetimedb::http::handler]
fn mta_hook_handler(ctx: &mut HandlerContext, request: HttpRequest) -> HttpResponse {
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

    let body_bytes: Vec<u8> = request.into_body().into_bytes().into();
    let mta_req: MtaHookRequest = match serde_json::from_slice(&body_bytes) {
        Ok(req) => req,
        Err(_) => {
            info!("Failed to parse MtaHookRequest");
            return json_response(400, json!({"error":"invalid JSON"}));
        }
    };

    match mta_req.context.stage {
        Stage::Data => {
            // persist message using the existing module routines in a transaction
            let _ = ctx.with_tx(|tx| {
                crate::services::mta::handle_data_stage(tx, &mta_req, tx.timestamp);
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
            ctx.with_tx(|tx| {
                if let Some(envelope) = &mta_req.envelope {
                    for recipient in &envelope.to {
                        let to_address = recipient.address.clone();
                        let topic_found = tx
                            .db
                            .message_topics()
                            .email_address()
                            .find(&to_address)
                            .map_or(false, |c| c.active);
                        let action_str =
                            if topic_found { "accept" } else { "reject" }.to_string();
                        tx.db.mta_connection_log().insert(MtaConnectionLog {
                            id: 0,
                            client_ip: "[REDACTED]".to_string(),
                            stage: "rcpt".to_string(),
                            action: action_str.clone(),
                            timestamp: tx.timestamp,
                            details: format!(
                                "Topic validation: {}",
                                if topic_found { "found" } else { "not found" }
                            ),
                        });
                    }
                }
            });

            let resp = MtaHookResponse::accept();

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
                    details: "Auth stage".to_string(),
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

#[derive(Deserialize)]
struct UnsubscribeRequest {
    token: String,
}

#[spacetimedb::http::handler]
fn mailing_list_unsubscribe_handler(
    ctx: &mut HandlerContext,
    request: HttpRequest,
) -> HttpResponse {
    let body_bytes: Vec<u8> = request.into_body().into_bytes().into();
    let payload: UnsubscribeRequest = match serde_json::from_slice(&body_bytes) {
        Ok(p) => p,
        Err(_) => return json_response(400, json!({"error":"invalid JSON"})),
    };

    let token_clone = payload.token.clone();
    let result = ctx.with_tx(|tx| unsubscribe_subscription_by_token(tx, token_clone.clone()));

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

    // Ensure any new or unprovisioned topics in the user's assignment are provisioned in Stalwart
    if payload.action == "upsert" {
        if let Some(topics) = &payload.user.topics {
            for topic in topics {
                let needs_provisioning = ctx.with_tx(|tx| {
                    match tx.db.message_topics().email_address().find(&topic.email_address) {
                        None => true,
                        Some(existing) => existing.app_password_id.is_none(),
                    }
                });

                if needs_provisioning {
                    info!(
                        "Provisioning Stalwart mailbox for topic '{}' ({})",
                        topic.name, topic.email_address
                    );
                    let visibility = match TopicVisibility::parse(&topic.visibility) {
                        Ok(v) => v,
                        Err(e) => {
                            return json_response(
                                400,
                                json!({"error": format!("Invalid topic visibility: {}", e)}),
                            );
                        }
                    };
                    let default_perm = match &topic.default_permission {
                        Some(p) => match SubscriptionPermission::parse(p) {
                            Ok(perm) => perm,
                            Err(e) => {
                                return json_response(
                                    400,
                                    json!({"error": format!("Invalid topic default_permission: {}", e)}),
                                );
                            }
                        },
                        None => SubscriptionPermission::Read,
                    };
                    if let Err(err) = provision_stalwart_topic_mailbox(
                        ctx,
                        &topic.name,
                        &topic.email_address,
                        &topic.description,
                        visibility,
                        default_perm,
                    ) {
                        log::error!(
                            "Failed to provision Stalwart mailbox for topic '{}': {}",
                            topic.email_address,
                            err
                        );
                        return json_response(
                            500,
                            json!({
                                "error": format!(
                                    "Failed to provision topic '{}': {}",
                                    topic.email_address, err
                                )
                            }),
                        );
                    }
                }
            }
        }
    }

    let result: Result<(), String> =
        ctx.with_tx(|tx| do_sync_user(tx, payload.action.clone(), user_data_str.clone()));

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

#[derive(Deserialize)]
struct TopicSyncPayload {
    action: String,
    topic: Option<TopicSyncData>,
    // Backwards compatibility alias during cutover
    category: Option<TopicSyncData>,
}

#[spacetimedb::http::handler]
fn topic_sync_handler(ctx: &mut HandlerContext, request: HttpRequest) -> HttpResponse {
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
    let payload: TopicSyncPayload = match serde_json::from_slice(&body_bytes) {
        Ok(p) => p,
        Err(_) => return json_response(400, json!({"error":"invalid JSON"})),
    };

    let topic_data = match payload.topic.or(payload.category) {
        Some(d) => d,
        None => return json_response(400, json!({"error":"missing topic in payload"})),
    };

    match payload.action.as_str() {
        "upsert" => {
            let topic = &topic_data;
            let visibility = match TopicVisibility::parse(&topic.visibility) {
                Ok(v) => v,
                Err(e) => {
                    return json_response(
                        400,
                        json!({"error": format!("Invalid topic visibility: {}", e)}),
                    );
                }
            };
            let default_perm = match &topic.default_permission {
                Some(p) => match SubscriptionPermission::parse(p) {
                    Ok(perm) => perm,
                    Err(e) => {
                        return json_response(
                            400,
                            json!({"error": format!("Invalid topic default_permission: {}", e)}),
                        );
                    }
                },
                None => SubscriptionPermission::Read,
            };

            let needs_provisioning = ctx.with_tx(|tx| {
                match tx.db.message_topics().email_address().find(&topic.email_address) {
                    None => true,
                    Some(existing) => existing.app_password_id.is_none(),
                }
            });

            if needs_provisioning {
                if let Err(err) = provision_stalwart_topic_mailbox(
                    ctx,
                    &topic.name,
                    &topic.email_address,
                    &topic.description,
                    visibility,
                    default_perm,
                ) {
                    log::error!(
                        "Failed to provision Stalwart mailbox for topic '{}': {}",
                        topic.email_address,
                        err
                    );
                    return json_response(
                        500,
                        json!({"error": format!("Failed to provision topic: {}", err)}),
                    );
                }
            } else {
                // Topic already has an app password; update editable metadata if changed
                ctx.with_tx(|tx| {
                    if let Some(existing) = tx
                        .db
                        .message_topics()
                        .email_address()
                        .find(&topic.email_address)
                    {
                        let mut updated = MessageTopic {
                            name: topic.name.clone(),
                            description: topic.description.clone(),
                            visibility,
                            ..existing
                        };
                        if topic.default_permission.is_some() {
                            updated.default_permission = default_perm;
                        }
                        tx.db.message_topics().id().update(updated);
                    }
                });
            }

            if let Some(categories) = &topic.categories {
                let topic_id = ctx.with_tx(|tx| {
                    tx.db
                        .message_topics()
                        .email_address()
                        .find(&topic.email_address)
                        .map(|c| c.id)
                });
                if let Some(topic_id) = topic_id {
                    let res = ctx.with_tx(|tx| {
                        crate::reducers::topics::sync_topic_categories(
                            tx,
                            topic_id,
                            categories.clone(),
                        )
                    });
                    if let Err(err) = res {
                        log::error!(
                            "Failed to sync categories for topic '{}': {}",
                            topic.email_address,
                            err
                        );
                    }
                }
            }

            json_response(
                200,
                json!({
                    "status": "success",
                    "action": "upsert",
                    "email_address": topic.email_address
                }),
            )
        }
        _ => json_response(400, json!({"error": format!("unsupported action '{}'", payload.action)})),
    }
}

#[spacetimedb::http::router]
fn router() -> Router {
    Router::new()
        .post("/mta-hook", mta_hook_handler)
        .post("/user-sync", user_sync_handler)
        .post("/topic-sync", topic_sync_handler)
        .post("/category-sync", topic_sync_handler)
        .post(
            "/mailing-list/unsubscribe",
            mailing_list_unsubscribe_handler,
        )
}
