use crate::module_bindings::{self, AccountTableAccess as _, DbConnection};

use base64::Engine;
use dioxus::{
    dioxus_core::SpawnIfAsync,
    logger::tracing::{debug, error, info},
    prelude::*,
};
use serde_json::Value as JsonValue;
use spacetimedb_sdk::{DbContext as _, Identity, Table as _};
use std::rc::Rc;

/// Connection state for SpacetimeDB
#[derive(Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected(Identity),
    Error(String),
}

/// Options for configuring the SpacetimeDB connection
#[derive(Clone, PartialEq, Debug)]
pub struct SpacetimeDbOptions {
    pub uri: String,
    pub module_name: String,
    pub token: Option<String>,
}

impl Default for SpacetimeDbOptions {
    fn default() -> Self {
        Self {
            uri: "http://localhost:3000".to_string(),
            module_name: "kommunikation".to_string(),
            token: None,
        }
    }
}

/// Return type for the `use_spacetime_db` hook
#[derive(Clone)]
pub struct SpacetimeDb {
    pub connection: Signal<Option<DbConnection>>,
    pub state: Signal<ConnectionState>,
    pub identity: Signal<Option<Identity>>,
    #[allow(dead_code)]
    pub connect: Rc<dyn Fn()>,
    #[allow(dead_code)]
    pub disconnect: Rc<dyn Fn()>,
}

/// Custom hook for SpacetimeDB connection management
pub fn use_spacetime_db(options: SpacetimeDbOptions) -> SpacetimeDb {
    let options_for_effect = options.clone();
    let conn_state = use_signal(|| ConnectionState::Disconnected);
    let identity = use_signal(|| {
        // Store the full number 415 as a little-endian u32 in the first 4 bytes of the array
        let mut arr = [0u8; 32];
        arr[..4].copy_from_slice(&415u32.to_le_bytes());
        Some(Identity::from_byte_array(arr))
    });
    let connection = use_signal(|| None::<DbConnection>);
    let is_connecting = use_signal(|| false);
    // Track last seen token to avoid unnecessary reconnects; None = uninitialized
    let last_token = use_signal(|| None::<Option<String>>);

    // Simple connect function without complex callbacks
    let connect = {
        Rc::new(move || {
            // Prevent multiple connection attempts
            let mut conn_state = conn_state;
            let identity = identity;
            let connection = connection;
            let mut is_connecting = is_connecting;
            let options = options.clone();

            if *is_connecting.read() || matches!(*conn_state.read(), ConnectionState::Connected(_))
            {
                return;
            }

            is_connecting.set(true);
            conn_state.set(ConnectionState::Connecting);
            info!("Starting SpacetimeDB connection attempt to {}", options.uri);

            // Spawn the connection attempt
            spawn({
                let mut conn_state = conn_state;
                let mut identity = identity;
                let mut connection = connection;
                let mut is_connecting = is_connecting;

                async move {
                    info!("Building SpacetimeDB connection...");
                    debug!("SpacetimeDB options: {:?}", options);

                    // Diagnostics: parse JWT timing to detect skew/expiry
                    if let Some(tok) = options.token.as_ref() {
                        if let Some((iat, exp)) = decode_jwt_times(tok) {
                            let now = (js_sys::Date::new_0().get_time() / 1000.0) as i64;
                            let expires_in = exp - now;
                            let skew_iat = now - iat;
                            info!("JWT timing: iat={}, exp={}, now={}, expires_in={}s, now_minus_iat={}s", iat, exp, now, expires_in, skew_iat);
                        } else {
                            debug!("JWT timing: unable to decode iat/exp");
                        }
                    }

                    // Build connection with primary token only
                    let conn_result = DbConnection::builder()
                        .with_uri(&options.uri)
                        .with_module_name(&options.module_name)
                        .with_token(options.token.clone())
                        .build()
                        .await;

                    match conn_result {
                        Ok(conn) => {
                            info!("DbConnection::builder().build() succeeded");
                            // Start the connection background processing
                            conn.run_background();

                            // Try to get the identity immediately
                            if let Some(id) = conn.try_identity() {
                                info!("Connection established with identity (immediate): {:?}", id);
                                conn_state.set(ConnectionState::Connected(id));
                                identity.set(Some(id));
                            } else {
                                info!("Identity not immediately available, will retry...");
                                // Create a minimal identity for state tracking
                                let dummy_identity =
                                    spacetimedb_sdk::Identity::from_byte_array([0u8; 32]);
                                conn_state.set(ConnectionState::Connected(dummy_identity));
                                identity.set(None);

                                // Spawn a task to periodically check for identity after setting the connection
                                spawn({
                                    let connection = connection;
                                    let mut identity = identity;
                                    let mut conn_state = conn_state;

                                    async move {
                                        use gloo_timers::future::TimeoutFuture;

                                        // Try for up to 10 seconds with 500ms intervals
                                        for attempt in 1..=20 {
                                            TimeoutFuture::new(500).await;

                                            if let Some(conn) = connection.read().as_ref() {
                                                if let Some(id) = conn.try_identity() {
                                                    info!(
                                                        "Identity obtained after {} attempts: {:?}",
                                                        attempt, id
                                                    );
                                                    conn_state.set(ConnectionState::Connected(id));
                                                    identity.set(Some(id));
                                                    break;
                                                } else {
                                                    info!("Identity check attempt {}/20 - still not available", attempt);
                                                }
                                            }
                                        }

                                        if let Some(conn) = connection.read().as_ref() {
                                            if conn.try_identity().is_none() {
                                                info!("Identity still not available after 10 seconds, proceeding without it");
                                            }
                                        }
                                    }
                                });
                            }

                            connection.set(Some(conn));
                            is_connecting.set(false);
                            info!("SpacetimeDB connection setup completed successfully");
                        }
                        Err(e) => {
                            error!("DbConnection::builder().build() failed: {:?}", e);
                            conn_state.set(ConnectionState::Error(e.to_string()));
                            is_connecting.set(false);
                        }
                    }
                }
            });
        })
    };

    // Disconnect function
    let disconnect = {
        Rc::new(move || {
            let mut conn_state = conn_state;
            let mut identity = identity;
            let mut connection = connection;

            if let Some(conn) = connection.read().as_ref() {
                let _ = conn.disconnect();
            }

            connection.set(None);
            conn_state.set(ConnectionState::Disconnected);
            identity.set(None);
        })
    };

    // Connect on mount
    use_effect({
        let connect = connect.clone();
        move || {
            connect();
        }
    });

    // Auto-reconnect when tokens change (options are captured by value into the hook via props)
    use_effect({
        let disconnect_cb = disconnect.clone();
        let connect_cb = connect.clone();
        // Snapshot of current token
        let dep_primary = options_for_effect.token.clone();
        let last_token = last_token.clone();
        move || {
            let current = Some(dep_primary.clone());
            // Only reconnect if token actually changed AND we were initialized before
            let prev = last_token.read().clone();
            if prev.is_none() {
                // Initialize without reconnect to avoid loop right after first connect
                let mut lt = last_token;
                lt.set(current);
            } else if prev != current {
                let mut lt = last_token;
                lt.set(current);
                debug!("Token change observed, reconnecting to SpacetimeDB...");
                (disconnect_cb)();
                let connect_cb2 = connect_cb.clone();
                spawn(async move {
                    gloo_timers::future::TimeoutFuture::new(50).await;
                    (connect_cb2)();
                });
            }
        }
    });

    SpacetimeDb {
        connection,
        state: conn_state,
        identity,
        connect,
        disconnect,
    }
}

// Decode iat/exp from a JWT (without verification). Returns (iat, exp) if present.
fn decode_jwt_times(token: &str) -> Option<(i64, i64)> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let payload_b64 = parts[1];
    // base64url decode
    let mut s = payload_b64.replace('-', "+").replace('_', "/");
    while s.len() % 4 != 0 {
        s.push('=');
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(s).ok()?;
    let payload: JsonValue = serde_json::from_slice(&bytes).ok()?;
    let iat = payload.get("iat").and_then(|v| v.as_i64())?;
    let exp = payload.get("exp").and_then(|v| v.as_i64())?;
    Some((iat, exp))
}

// Extension hook for subscribing to tables
pub fn use_spacetime_subscription(
    spacetime_db: &SpacetimeDb,
    queries: Vec<String>,
) -> Signal<bool> {
    let is_subscribed = use_signal(|| false);

    // Clone signals for the effect dependency tracking
    let state = spacetime_db.state;
    let connection = spacetime_db.connection;
    let mut is_subscribed_clone = is_subscribed;

    // Subscribe when connection becomes available
    use_effect(move || {
        if let ConnectionState::Connected(_) = *state.read() {
            if let Some(conn) = connection.read().as_ref() {
                is_subscribed_clone.set(false);

                // Simple subscription without callbacks for now
                let _subscription_result = conn
                    .subscription_builder()
                    .subscribe(queries.clone())
                    .spawn();

                // For now, just assume success
                info!("Subscribed to queries: {:?}", queries);
                is_subscribed_clone.set(true);

                // Log basic table info immediately after subscription
                let count = conn.db().account().count();
                info!("Subscription established. Current account count: {}", count);

                // Also try to list all accounts to debug
                let accounts: Vec<_> = conn.db().account().iter().collect();
                info!("All accounts after subscription: {:?}", accounts);
            }
        } else {
            is_subscribed_clone.set(false);
        }
    });

    is_subscribed
}

// Hook to get table data that automatically updates
pub fn use_table_data<T>(
    spacetime_db: &SpacetimeDb,
    table_getter: impl Fn(&module_bindings::DbConnection) -> Vec<T> + 'static + Clone,
) -> Signal<Vec<T>>
where
    T: Clone + 'static,
{
    let data = use_signal(|| Vec::<T>::new());

    // Clone for the effect
    let state = spacetime_db.state;
    let connection = spacetime_db.connection;
    let mut data_clone = data;
    let table_getter_clone = table_getter.clone();

    // Load data whenever connection state changes
    use_effect(move || {
        if let ConnectionState::Connected(_) = *state.read() {
            if let Some(conn) = connection.read().as_ref() {
                let new_data = table_getter_clone(conn);
                info!("Table data loaded: {} items", new_data.len());

                // Add more detailed logging
                info!("DB connection valid: {}", conn.db().account().count());
                let all_accounts: Vec<_> = conn.db().account().iter().collect();
                info!("All accounts from effect: {:?}", all_accounts);

                data_clone.set(new_data);
            }
        } else {
            info!("Connection not ready, clearing table data");
            data_clone.set(Vec::new());
        }
    });

    // Also setup a timer to periodically refresh data
    // This ensures we catch changes from subscriptions
    use_effect({
        let state = spacetime_db.state;
        let connection = spacetime_db.connection;
        let data_clone = data;
        let table_getter_clone = table_getter;

        move || {
            // Create a periodic refresh for connected state
            if matches!(*state.read(), ConnectionState::Connected(_)) {
                spawn({
                    let state = state;
                    let connection = connection;
                    let mut data_clone = data_clone;
                    let table_getter_clone = table_getter_clone.clone();

                    async move {
                        loop {
                            gloo_timers::future::TimeoutFuture::new(2000).await; // Wait 2000 milliseconds
                            if let ConnectionState::Connected(_) = *state.read() {
                                if let Some(conn) = connection.read().as_ref() {
                                    info!("Refreshing table data...");
                                    let new_data = table_getter_clone(conn);
                                    info!("fetched new table data: {} items", new_data.len());

                                    // More debugging
                                    let db_count = conn.db().account().count();
                                    info!("Direct DB account count: {}", db_count);

                                    if new_data.len() != data_clone.read().len() {
                                        info!(
                                            "Table data refreshed: {} items (was {})",
                                            new_data.len(),
                                            data_clone.read().len()
                                        );
                                        data_clone.set(new_data);
                                    }
                                }
                            } else {
                                info!("Not connected, breaking refresh loop");
                                break; // Stop refreshing if disconnected
                            }
                        }
                    }
                });
            }
        }
    });

    data
}

// Convenience hook specifically for accounts
pub fn use_accounts_table(
    spacetime_db: &SpacetimeDb,
) -> Signal<Vec<crate::module_bindings::Account>> {
    use_table_data(spacetime_db, |conn| conn.db().account().iter().collect())
}
