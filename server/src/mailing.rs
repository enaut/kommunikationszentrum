use log::{error, info};
use spacetimedb::{Query, ReducerContext, Table, Timestamp, ViewContext};

use crate::account::{account, account__view, admin_identities__view, is_admin_user, Account};

#[spacetimedb::table(accessor = message_categories, public)]
pub struct MessageCategory {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub name: String,
    #[unique]
    pub email_address: String,
    pub description: String,
    pub active: bool,
}

#[derive(Clone)]
#[spacetimedb::table(accessor = subscriptions, public)]
pub struct Subscription {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[index(btree)]
    pub subscriber_account_id: u64,
    #[index(btree)]
    pub subscriber_email: String,
    #[index(btree)]
    pub category_id: u64,
    pub subscribed_at: Timestamp,
    pub active: bool,
}

#[derive(Clone)]
#[spacetimedb::table(accessor = subscription_unsubscribe_tokens, public)]
pub struct SubscriptionUnsubscribeToken {
    #[primary_key]
    pub token: String,
    #[unique]
    pub subscription_id: u64,
    #[index(btree)]
    pub created_at: Timestamp,
    pub active: bool,
    pub revoked_at: Timestamp,
}

/// Returns all subscriptions for admins; only the caller's own subscriptions for regular users.
/// Clients subscribe to this view instead of the raw `subscriptions` table.
#[spacetimedb::view(accessor = visible_subscriptions, public)]
pub fn visible_subscriptions(ctx: &ViewContext) -> Vec<Subscription> {
    let sender = ctx.sender();
    let is_admin = ctx.db.admin_identities().identity().find(&sender).is_some();
    if is_admin {
        ctx.db
            .subscriptions()
            .subscriber_account_id()
            .filter(0u64..)
            .collect()
    } else {
        match ctx.db.account().identity().find(&sender) {
            Some(acc) => ctx
                .db
                .subscriptions()
                .subscriber_account_id()
                .filter(&acc.id)
                .collect(),
            None => vec![],
        }
    }
}

#[spacetimedb::view(accessor = active_subscriptions, public)]
pub fn active_subscriptions(ctx: &ViewContext) -> impl Query<Subscription> {
    ctx.from.subscriptions().r#filter(|sub| sub.active)
}

#[spacetimedb::view(accessor = active_unsubscribe_tokens, public)]
pub fn active_unsubscribe_tokens(ctx: &ViewContext) -> impl Query<SubscriptionUnsubscribeToken> {
    ctx.from
        .subscription_unsubscribe_tokens()
        .r#filter(|token| token.active)
}

#[spacetimedb::reducer]
pub fn add_message_category(
    ctx: &ReducerContext,
    name: String,
    email_address: String,
    description: String,
) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }

    ctx.db.message_categories().insert(MessageCategory {
        id: 0,
        name,
        email_address,
        description,
        active: true,
    });
    log::info!(
        "Added new message category (by identity: {:?})",
        ctx.sender()
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn remove_message_category(ctx: &ReducerContext, category_id: u64) -> Result<(), String> {
    if !is_admin_user(ctx) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    if ctx
        .db
        .message_categories()
        .id()
        .find(&category_id)
        .is_none()
    {
        return Err(format!("Message category {} not found", category_id));
    }
    ctx.db.message_categories().id().delete(&category_id);
    log::info!(
        "Removed message category {} (by identity: {:?})",
        category_id,
        ctx.sender()
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn add_subscription(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    subscriber_email: String,
    category_id: u64,
) -> Result<(), String> {
    let is_admin = is_admin_user(ctx);
    let is_self = ctx
        .db
        .account()
        .id()
        .find(&subscriber_account_id)
        .map(|a: Account| a.identity == ctx.sender())
        .unwrap_or(false);

    if !is_admin && !is_self {
        return Err("Unauthorized: can only subscribe yourself or requires admin".to_string());
    }

    let timestamp = ctx.timestamp;

    let existing = ctx
        .db
        .subscriptions()
        .subscriber_account_id()
        .filter(&subscriber_account_id)
        .find(|sub| sub.category_id == category_id);

    let subscription = if let Some(existing) = existing {
        let updated = Subscription {
            subscriber_email: subscriber_email.clone(),
            subscribed_at: timestamp,
            active: true,
            ..existing
        };
        ctx.db.subscriptions().id().update(updated.clone());
        updated
    } else {
        let candidate = Subscription {
            id: 0,
            subscriber_account_id,
            subscriber_email: subscriber_email.clone(),
            category_id,
            subscribed_at: timestamp,
            active: true,
        };
        ctx.db.subscriptions().insert(candidate);
        ctx.db
            .subscriptions()
            .subscriber_account_id()
            .filter(&subscriber_account_id)
            .find(|sub| sub.category_id == category_id)
            .ok_or_else(|| "Subscription insert failed".to_string())?
    };

    let token = upsert_subscription_unsubscribe_token(ctx, subscription.id)?;
    log::info!(
        "Added subscription for account {} (token: {}, by identity: {:?})",
        subscriber_account_id,
        token,
        ctx.sender()
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn remove_subscription(ctx: &ReducerContext, subscription_id: u64) -> Result<(), String> {
    let sub = ctx
        .db
        .subscriptions()
        .id()
        .find(&subscription_id)
        .ok_or_else(|| format!("Subscription {} not found", subscription_id))?;

    let is_admin = is_admin_user(ctx);
    let is_self = ctx
        .db
        .account()
        .id()
        .find(&sub.subscriber_account_id)
        .map(|a| a.identity == ctx.sender())
        .unwrap_or(false);

    if !is_admin && !is_self {
        return Err(
            "Unauthorized: can only remove your own subscriptions or requires admin".to_string(),
        );
    }

    let mut updated = sub.clone();
    updated.active = false;
    ctx.db.subscriptions().id().update(updated);
    deactivate_subscription_unsubscribe_token(ctx, subscription_id);
    log::info!(
        "Deactivated subscription {} (by identity: {:?})",
        subscription_id,
        ctx.sender()
    );
    Ok(())
}

fn upsert_subscription_unsubscribe_token(
    ctx: &ReducerContext,
    subscription_id: u64,
) -> Result<String, String> {
    if let Some(existing) = ctx
        .db
        .subscription_unsubscribe_tokens()
        .subscription_id()
        .find(&subscription_id)
    {
        if existing.active {
            return Ok(existing.token);
        }

        let mut updated = existing.clone();
        updated.active = true;
        updated.revoked_at = Timestamp::UNIX_EPOCH;
        updated.created_at = ctx.timestamp;
        ctx.db
            .subscription_unsubscribe_tokens()
            .token()
            .update(updated.clone());
        return Ok(updated.token);
    }

    let token = format!("sub-{subscription_id}-{:032x}", ctx.random::<u128>());
    ctx.db
        .subscription_unsubscribe_tokens()
        .insert(SubscriptionUnsubscribeToken {
            token: token.clone(),
            subscription_id,
            created_at: ctx.timestamp,
            active: true,
            revoked_at: Timestamp::UNIX_EPOCH,
        });
    Ok(token)
}

fn deactivate_subscription_unsubscribe_token(ctx: &ReducerContext, subscription_id: u64) {
    if let Some(existing) = ctx
        .db
        .subscription_unsubscribe_tokens()
        .subscription_id()
        .find(&subscription_id)
    {
        let mut updated = existing.clone();
        updated.active = false;
        updated.revoked_at = ctx.timestamp;
        ctx.db
            .subscription_unsubscribe_tokens()
            .token()
            .update(updated);
    }
}

#[spacetimedb::reducer]
pub fn ensure_subscription_unsubscribe_token(
    ctx: &ReducerContext,
    subscription_id: u64,
) -> Result<(), String> {
    upsert_subscription_unsubscribe_token(ctx, subscription_id).map(|_| ())
}

pub(crate) fn unsubscribe_subscription_by_token(
    ctx: &ReducerContext,
    token: String,
) -> Result<(), String> {
    let token_row = ctx
        .db
        .subscription_unsubscribe_tokens()
        .token()
        .find(&token)
        .ok_or_else(|| "Unknown unsubscribe token".to_string())?;

    let Some(subscription) = ctx.db.subscriptions().id().find(&token_row.subscription_id) else {
        return Err("Subscription missing for token".to_string());
    };

    if !subscription.active {
        return Ok(());
    }

    let mut updated_subscription = subscription.clone();
    updated_subscription.active = false;
    ctx.db.subscriptions().id().update(updated_subscription);
    deactivate_subscription_unsubscribe_token(ctx, token_row.subscription_id);
    Ok(())
}

// Procedure: Provision a Stalwart mailbox via JMAP and insert the message category on success.
#[spacetimedb::procedure]
pub fn provision_message_category(
    ctx: &mut spacetimedb::ProcedureContext,
    name: String,
    email_address: String,
    description: String,
) -> Result<(), String> {
    use crate::account::admin_identities;
    info!(
        "Provisioning a new Category: {}, {}, {}",
        name, email_address, description
    );
    // 1) Authorization check: capture the procedure caller identity and check inside a transaction
    let caller = ctx.sender();
    info!("Checking permissions for identity: {:?}", caller);
    let is_admin: bool = ctx.with_tx(|tx| {
        if caller == tx.database_identity() {
            return true;
        }
        tx.db.admin_identities().identity().find(&caller).is_some()
    });

    if !is_admin {
        return Err("Unauthorized: Admin access required".to_string());
    }

    info!("User has required permissions!");

    // 2) Ensure category doesn't already exist
    let exists: bool = ctx.with_tx(|tx| {
        tx.db
            .message_categories()
            .email_address()
            .find(&email_address)
            .is_some()
    });

    if exists {
        error!("The category with that mailadress already exists");
        return Err(format!(
            "Category with email {} already exists",
            email_address
        ));
    }

    // 3) Read compile-time configuration for JMAP URL and admin token
    let jmap_base = env!("STALWART_JMAP_URL");
    let admin_token = env!("STALWART_ADMIN_TOKEN");

    let endpoint = if jmap_base.ends_with("/jmap") {
        jmap_base.trim_end_matches('/').to_string()
    } else {
        format!("{}/jmap", jmap_base.trim_end_matches('/'))
    };

    // 4) Build JMAP payload
    let create_map = serde_json::json!({
        "create": {
            "create-1": {
                "@type": "User",
                "name": email_address.split("@").next(),
                "description": name,
                "domainId": "c",
                "roles": {
                  "@type": "User"
                },
                "permissions": {
                  "@type": "Inherit"
                },
                "aliases": {},
                "memberGroupIds": {},
                "quotas": {},
                "credentials": {},
                "encryptionAtRest": {
                  "@type": "Disabled"
                }
            }
        }
    });

    let payload = serde_json::json!({
        "using": [
            "urn:ietf:params:jmap:core",
            "urn:stalwart:jmap"
        ],
        "methodCalls": [
            ["x:Account/set", create_map, "call-id-1"]
        ]
    });

    let body = serde_json::to_vec(&payload).map_err(|e| {
        error!("Failed to serialize JMAP payload: {}", e);
        format!("Failed to serialize JMAP payload: {}", e)
    })?;

    info!("body created!");

    let request = spacetimedb::http::Request::builder()
        .uri(endpoint)
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", admin_token))
        .extension(spacetimedb::http::Timeout(
            spacetimedb::TimeDuration::from_micros(30_000_000),
        ))
        .body(body)
        .map_err(|e| format!("Failed to build HTTP request: {:?}", e))?;
    info!("request created!");
    // 5) Perform HTTP request
    let response = ctx.http.send(request).map_err(|e| {
        error!("Failed to perform request: {}", e);
        format!("HTTP send failed: {:?}", e)
    })?;

    info!("Response: {:?}", response.status());

    let (parts, body) = response.into_parts();

    if parts.status != 200 {
        let body = body.into_string_lossy();
        error!(
            "Stalwart responded with status {} and body {}",
            parts.status, body
        );
        return Err(format!(
            "Stalwart responded with status {} and body {}",
            parts.status, body
        ));
    }

    let body_bytes = body.into_bytes();
    let res_body: serde_json::Value = serde_json::from_slice(&body_bytes)
        .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

    info!("Response Body: {}", res_body);

    // Inspect methodResponses for x:Account/set and check for `notCreated`
    if let Some(method_responses) = res_body.get("methodResponses").and_then(|v| v.as_array()) {
        for entry in method_responses {
            if let Some(method_name) = entry.get(0).and_then(|v| v.as_str()) {
                if method_name == "x:Account/set" {
                    if let Some(result_obj) = entry.get(1) {
                        if let Some(not_created) = result_obj.get("notCreated") {
                            if not_created
                                .as_object()
                                .map(|m| !m.is_empty())
                                .unwrap_or(false)
                            {
                                return Err(format!("JMAP reported notCreated: {}", not_created));
                            }
                        }
                        // Success path: insert the category inside a transaction
                        ctx.with_tx(|tx| {
                            tx.db.message_categories().insert(MessageCategory {
                                id: 0,
                                name: name.clone(),
                                email_address: email_address.clone(),
                                description: description.clone(),
                                active: true,
                            });
                        });

                        return Ok(());
                    }
                }
            }
        }
    }

    Err(format!("Unexpected JMAP response: {}", res_body))
}
