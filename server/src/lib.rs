use serde::{Deserialize, Serialize};
use spacetimedb::{Filter, Identity, ReducerContext, Table};
use stalwart_mta_hook_types::{Request as MtaHookRequest, Stage};

// Configuration constants that can be set at compile time via environment variables
const DJANGO_OAUTH_BASE_URL: &str = match option_env!("DJANGO_BASE_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:8000",
};

const DJANGO_OAUTH_ISSUER_PATH: &str = "/o";

#[derive(Debug)]
#[spacetimedb::table(name = account, public)]
pub struct Account {
    #[primary_key]
    pub id: u64, // mitgliedsnr from Django
    pub identity: Identity,
    pub name: String,
    pub email: String,
    pub is_active: bool,
    pub is_admin: bool, // RLS: Admins dürfen alle Accounts sehen
    pub last_synced: i64,
}

#[spacetimedb::table(name = admin_identities)]
pub struct AdminIdentity {
    #[primary_key]
    pub identity: Identity,
}

#[spacetimedb::table(name = mta_connection_log)]
pub struct MtaConnectionLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub client_ip: String,
    pub stage: String,
    pub action: String,
    pub timestamp: i64,
    pub details: String,
}

#[spacetimedb::table(name = mta_message_log)]
pub struct MtaMessageLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub from_address: String,
    pub to_addresses: String, // JSON array as string
    pub subject: String,
    pub message_size: u64,
    pub stage: String,
    pub action: String,
    pub timestamp: i64,
    pub queue_id: Option<String>,
}

#[spacetimedb::table(name = blocked_ips)]
pub struct BlockedIp {
    #[primary_key]
    pub ip: String,
    pub reason: String,
    pub blocked_at: i64,
    pub active: bool,
}

#[spacetimedb::table(name = message_categories)]
pub struct MessageCategory {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub name: String,
    pub email_address: String,
    pub description: String,
    pub active: bool,
}

#[spacetimedb::table(name = subscriptions)]
pub struct Subscription {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub subscriber_account_id: u64,
    pub subscriber_email: String,
    pub category_id: u64,
    pub subscribed_at: i64,
    pub active: bool,
}

#[derive(Serialize, Deserialize)]
pub struct UserSyncPayload {
    pub action: String, // "upsert" or "delete"
    pub user: UserSyncData,
}

#[derive(Serialize, Deserialize)]
pub struct UserSyncData {
    pub mitgliedsnr: u64,
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<bool>,
    pub updated_at: Option<String>,
    // Optional: precomputed Spacetime Identity as hex string (provided by Django)
    pub identity_hex: Option<String>,
}

#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) {
    // Called when the module is initially published
    let module_id = ctx.identity();
    let mut exists = false;
    for row in ctx.db.admin_identities().iter() {
        if row.identity == module_id {
            exists = true;
            break;
        }
    }
    if !exists {
        ctx.db.admin_identities().insert(AdminIdentity {
            identity: module_id,
        });
        log::info!("Seeded module identity as admin: {:?}", module_id);
    }

    // Insert a test account row to verify writes work after publish
    let timestamp = ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000;
    ctx.db.account().insert(Account {
        id: 999999,
        identity: module_id,
        name: "Init Test".to_string(),
        email: "init@test.local".to_string(),
        is_active: true,
        is_admin: true,
        last_synced: timestamp,
    });
    log::info!("Inserted init test account row");
}

#[spacetimedb::reducer(client_connected)]
pub fn identity_connected(ctx: &ReducerContext) {
    // Called everytime a new client connects
    // Check if the connected identity corresponds to a known account
    log::info!("Client connected with identity: {:?}", ctx.sender);

    // You could implement logic here to:
    // 1. Check if this identity is authorized
    // 2. Link the identity to an account in the database
    // 3. Set up user-specific permissions

    // Example: Check if the sender has admin privileges
    // This would be based on the JWT claims that SpacetimeDB validated
}

// ------------------------------------------------------------
// Row Level Security (Client Visibility)
// Normale Benutzer sollen nur ihren eigenen Account sehen.
// Admin Benutzer (is_admin=true) sehen alle Accounts.
// Diese Funktion wird (unstable Feature) vom SpacetimeDB Runtime
// zur Filterung pro Zeile aufgerufen.
// Hinweis: Falls der Attributname sich geändert hat, bitte in der
// SpacetimeDB Doku nach dem korrekten "client visibility" Attribut
// suchen und anpassen.
// Client Visibility Filter (Row Level Security):
// Self-only visibility: clients can only see their own account row
#[spacetimedb::client_visibility_filter]
pub const ACCOUNT_VISIBILITY: Filter =
    Filter::Sql("SELECT * FROM account where identity = :sender");

#[spacetimedb::reducer(client_disconnected)]
pub fn identity_disconnected(_ctx: &ReducerContext) {
    // Called everytime a client disconnects
}

#[spacetimedb::reducer]
pub fn handle_mta_hook(ctx: &ReducerContext, hook_data: String) {
    match serde_json::from_str::<MtaHookRequest>(&hook_data) {
        Ok(request) => {
            let timestamp = ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000; // Convert to seconds as i64

            match request.context.stage {
                Stage::Connect => handle_connect_stage(ctx, &request, timestamp),
                Stage::Ehlo => handle_ehlo_stage(ctx, &request, timestamp),
                Stage::Mail => handle_mail_stage(ctx, &request, timestamp),
                Stage::Rcpt => handle_rcpt_stage(ctx, &request, timestamp),
                Stage::Data => handle_data_stage(ctx, &request, timestamp),
                Stage::Auth => handle_auth_stage(ctx, &request, timestamp),
            }
        }
        Err(e) => {
            log::error!("Failed to parse MTA hook data: {}", e);
        }
    }
}

fn handle_connect_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: i64) {
    let client_ip = &request.context.client.ip;

    log::info!("Connect stage - IP: [REDACTED]");

    // Check if IP is blocked
    for blocked_ip in ctx.db.blocked_ips().iter() {
        if blocked_ip.ip == *client_ip && blocked_ip.active {
            log::warn!("Blocked connection from IP");

            ctx.db.mta_connection_log().insert(MtaConnectionLog {
                id: 0,
                client_ip: "[REDACTED]".to_string(),
                stage: "connect".to_string(),
                action: "reject".to_string(),
                timestamp,
                details: "IP blocked".to_string(),
            });
            return;
        }
    }

    ctx.db.mta_connection_log().insert(MtaConnectionLog {
        id: 0,
        client_ip: client_ip.to_string(),
        stage: "connect".to_string(),
        action: "accept".to_string(),
        timestamp,
        details: "Connection accepted".to_string(),
    });
}

fn handle_ehlo_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: i64) {
    let client_ip = "[REDACTED]";
    let helo = request.context.client.helo.as_deref().unwrap_or("unknown");

    log::info!("EHLO stage - HELO: [REDACTED]");

    // Basic EHLO validation
    let is_valid = !helo.is_empty() && helo != "unknown";
    let action = if is_valid { "accept" } else { "reject" };
    let details = if is_valid {
        "Valid EHLO/HELO".to_string()
    } else {
        "Invalid EHLO/HELO".to_string()
    };

    ctx.db.mta_connection_log().insert(MtaConnectionLog {
        id: 0,
        client_ip: client_ip.to_string(),
        stage: "ehlo".to_string(),
        action: action.to_string(),
        timestamp,
        details,
    });
}

fn handle_mail_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: i64) {
    let from_address = request
        .envelope
        .as_ref()
        .map(|env| env.from.address.as_str())
        .unwrap_or("unknown");

    log::info!("MAIL stage - From: [REDACTED]");

    // Basic sender validation
    let is_valid = from_address.contains('@') && !from_address.is_empty();
    let action = if is_valid { "accept" } else { "reject" };
    let details = format!(
        "Sender validation: {}",
        if is_valid { "passed" } else { "failed" }
    );

    ctx.db.mta_connection_log().insert(MtaConnectionLog {
        id: 0,
        client_ip: "[REDACTED]".to_string(),
        stage: "mail".to_string(),
        action: action.to_string(),
        timestamp,
        details,
    });
}

fn handle_rcpt_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: i64) {
    if let Some(envelope) = &request.envelope {
        for recipient in &envelope.to {
            let to_address = recipient.address.as_str();
            log::info!("RCPT stage - To: [REDACTED]");

            // Check if recipient corresponds to a valid category
            let mut category_found = false;
            for category in ctx.db.message_categories().iter() {
                if category.email_address == to_address && category.active {
                    category_found = true;
                    break;
                }
            }

            let action = if category_found { "accept" } else { "reject" };
            let details = format!(
                "Category validation: {}",
                if category_found { "found" } else { "not found" }
            );

            ctx.db.mta_connection_log().insert(MtaConnectionLog {
                id: 0,
                client_ip: "[REDACTED]".to_string(),
                stage: "rcpt".to_string(),
                action: action.to_string(),
                timestamp,
                details,
            });
        }
    }
}

fn handle_data_stage(ctx: &ReducerContext, request: &MtaHookRequest, timestamp: i64) {
    let from_address = request
        .envelope
        .as_ref()
        .map(|env| env.from.address.as_str())
        .unwrap_or("unknown");
    let message_size = request
        .message
        .as_ref()
        .map(|msg| msg.size as u64)
        .unwrap_or(0);
    let subject = extract_subject_from_request(request);

    log::info!(
        "DATA stage - From: [REDACTED], Size: {}, Subject: [REDACTED]",
        message_size
    );

    let mut to_addresses = Vec::new();
    let mut valid_categories = Vec::new();

    if let Some(envelope) = &request.envelope {
        for recipient in &envelope.to {
            let to_address = recipient.address.as_str();
            to_addresses.push(to_address.to_string());

            // Find category for this recipient
            for category in ctx.db.message_categories().iter() {
                if category.email_address == to_address && category.active {
                    // Check if sender is subscribed
                    let mut is_subscribed = false;
                    for subscription in ctx.db.subscriptions().iter() {
                        if subscription.subscriber_email == from_address
                            && subscription.category_id == category.id
                            && subscription.active
                        {
                            is_subscribed = true;
                            break;
                        }
                    }

                    if is_subscribed {
                        valid_categories.push(category.id);
                    } else {
                        log::info!("Sender not subscribed to category: [REDACTED]");
                    }
                    break;
                }
            }
        }
    }

    let action = if !valid_categories.is_empty() {
        "accept"
    } else {
        "quarantine"
    };

    ctx.db.mta_message_log().insert(MtaMessageLog {
        id: 0,
        from_address: "[REDACTED]".to_string(),
        to_addresses: serde_json::to_string(&to_addresses).unwrap_or_default(),
        subject: if subject.len() > 100 {
            "[REDACTED]".to_string()
        } else {
            subject
        },
        message_size,
        stage: "data".to_string(),
        action: action.to_string(),
        timestamp,
        queue_id: request.context.queue.as_ref().map(|q| q.id.clone()),
    });
}

fn handle_auth_stage(ctx: &ReducerContext, _request: &MtaHookRequest, timestamp: i64) {
    log::info!("AUTH stage - accepting");

    ctx.db.mta_connection_log().insert(MtaConnectionLog {
        id: 0,
        client_ip: "[REDACTED]".to_string(),
        stage: "auth".to_string(),
        action: "accept".to_string(),
        timestamp,
        details: "Authentication stage - accept".to_string(),
    });
}

fn extract_subject_from_request(request: &MtaHookRequest) -> String {
    if let Some(message) = &request.message {
        for (name, value) in &message.headers {
            if name.to_lowercase() == "subject" {
                return value.trim().to_string();
            }
        }
    }
    "No subject".to_string()
}

// User synchronization from Django
#[spacetimedb::reducer]
pub fn sync_user(ctx: &ReducerContext, action: String, user_data: String) {
    // TEMP: Autorisierung deaktiviert, damit der Webhook-Proxy ohne Token synchronisieren kann.
    // WICHTIG: Für Produktion wieder absichern (is_admin_identity o.ä.).

    let timestamp = ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000;

    log::info!("Syncing user with action: {}", action);
    log::info!("User data: {}", user_data);

    match serde_json::from_str::<UserSyncData>(&user_data) {
        Ok(data) => {
            match action.as_str() {
                "upsert" => {
                    // Check if account exists and delete it first
                    log::info!("Syncing user: {} ({})", data.mitgliedsnr, action);
                    for existing in ctx.db.account().iter() {
                        log::info!("Checking existing account: {}", existing.id);
                        if existing.id == data.mitgliedsnr {
                            ctx.db.account().delete(existing);
                            break;
                        }
                    }
                    log::info!("No existing account found, proceeding to insert new account");

                    // Prefer provided identity_hex; if missing or parsing unsupported, fall back to module identity
                    let mitgliedsnr = data.mitgliedsnr.to_string();
                    let issuer_url = format!("{}{}", DJANGO_OAUTH_BASE_URL, DJANGO_OAUTH_ISSUER_PATH);

                    let identity_of_user =
                        Identity::from_claims(&issuer_url, &mitgliedsnr);
                    // Insert new/updated account
                    let account = Account {
                        id: data.mitgliedsnr,
                        identity: identity_of_user,
                        name: data.name.unwrap_or_default(),
                        email: data.email.unwrap_or_default(),
                        is_active: data.is_active.unwrap_or(true),
                        is_admin: false,
                        last_synced: timestamp,
                    };

                    log::info!("Inserting account: {:#?}", account);

                    let account = ctx.db.account().insert(account);
                    log::info!(
                        "Synced user: {} ({}) [result: {:?}]",
                        account.id,
                        action,
                        account
                    );
                }
                "delete" => {
                    // Find and delete the account
                    for existing in ctx.db.account().iter() {
                        if existing.id == data.mitgliedsnr {
                            ctx.db.account().delete(existing);
                            log::info!("Deleted user: {} ({})", data.mitgliedsnr, action);
                            break;
                        }
                    }
                }
                _ => {
                    log::warn!("Unknown sync action: {}", action);
                }
            }
        }
        Err(e) => {
            log::error!("Failed to parse user sync data: {}", e);
        }
    }
}

/// Check if the current user has admin permissions
fn is_admin_user(ctx: &ReducerContext) -> bool {
    is_admin_identity(ctx, ctx.sender)
}

/// True if the provided identity is the module owner or listed in admin_identities
fn is_admin_identity(ctx: &ReducerContext, who: Identity) -> bool {
    // Module owner is always admin
    if who == ctx.identity() {
        return true;
    }
    for row in ctx.db.admin_identities().iter() {
        if row.identity == who {
            return true;
        }
    }
    false
}

#[spacetimedb::reducer]
pub fn add_message_category(
    ctx: &ReducerContext,
    name: String,
    email_address: String,
    description: String,
) -> Result<(), String> {
    // Check if user is authenticated and has admin rights
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
    log::info!("Added new message category (by identity: {:?})", ctx.sender);
    Ok(())
}

#[spacetimedb::reducer]
pub fn add_subscription(
    ctx: &ReducerContext,
    subscriber_account_id: u64,
    subscriber_email: String,
    category_id: u64,
) {
    let timestamp = ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000; // Convert to seconds as i64

    ctx.db.subscriptions().insert(Subscription {
        id: 0,
        subscriber_account_id,
        subscriber_email,
        category_id,
        subscribed_at: timestamp,
        active: true,
    });
    log::info!("Added new subscription");
}

#[spacetimedb::reducer]
pub fn block_ip(ctx: &ReducerContext, ip: String, reason: String) {
    let timestamp = ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000; // Convert to seconds as i64

    ctx.db.blocked_ips().insert(BlockedIp {
        ip,
        reason,
        blocked_at: timestamp,
        active: true,
    });
    log::info!("Blocked IP address");
}

#[spacetimedb::reducer]
pub fn get_mta_logs(ctx: &ReducerContext) {
    log::info!("=== MTA Connection Logs ===");
    for log in ctx.db.mta_connection_log().iter() {
        log::info!(
            "Connection Log {}: {} - {} - {}",
            log.id,
            log.stage,
            log.action,
            log.details
        );
    }

    log::info!("=== MTA Message Logs ===");
    for log in ctx.db.mta_message_log().iter() {
        log::info!(
            "Message Log {}: {} - {} - Size: {}",
            log.id,
            log.stage,
            log.action,
            log.message_size
        );
    }
}

// Test reducer to add sample data
#[spacetimedb::reducer]
pub fn add_test_accounts(ctx: &ReducerContext) {
    let timestamp = ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000;

    // Add some test accounts
    ctx.db.account().insert(Account {
        id: 1,
        identity: ctx.identity(),
        name: "Test User 1".to_string(),
        email: "test1@example.com".to_string(),
        is_active: true,
        is_admin: false,
        last_synced: timestamp,
    });

    ctx.db.account().insert(Account {
        id: 2,
        identity: ctx.identity(),
        name: "Test User 2".to_string(),
        email: "test2@example.com".to_string(),
        is_active: true,
        is_admin: false,
        last_synced: timestamp,
    });

    log::info!("Added test accounts");
}
