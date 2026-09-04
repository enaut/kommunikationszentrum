pub mod auth_flow;
pub mod jwt_utils;
pub mod token_storage;

use crate::config::OAuthConfig;
use auth_flow::{
    attempt_refresh, build_user_info_from_openid, clear_url, create_oidc_client, get_http_client,
    initiate_login, initiate_logout, parse_url_params, schedule_refresh, OpenIdClient,
};
use dioxus::prelude::*;
use js_sys::Date;
pub use jwt_utils::DecodedJwt;
use openidconnect::{AuthorizationCode, Nonce, OAuth2TokenResponse, PkceCodeVerifier};
use serde::{Deserialize, Serialize};
pub use token_storage::*;
use tracing::warn;

#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    Unauthenticated,
    Authenticating,
    Authenticated(UserInfo),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub email: Option<String>,
    pub access_token: String,
    pub id_token: Option<String>,      // JWT for SpacetimeDB auth
    pub refresh_token: Option<String>, // Für stille Erneuerung
    pub mitgliedsnr: String,           // Subject from JWT (Mitgliedsnummer)
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub name: Option<String>,
    pub is_staff: Option<bool>,
    pub is_superuser: Option<bool>,
    pub groups: Option<Vec<String>>,
}

impl UserInfo {
    /// Decode the stored ID token (JWS) without verifying the signature.
    ///
    /// NOTE: This is a base64url decode + JSON parse only. Do not rely on this for security
    /// decisions; signature and claim validation must already have been done during login.
    pub fn decode_id_token(&self) -> Option<Result<DecodedJwt, String>> {
        let token = self.id_token.as_ref()?;
        Some(jwt_utils::decode_jwt(token))
    }
}

/// OAuth / OIDC hook for Dioxus components.
///
/// Manages authentication state, persists tokens across sessions, and caches the OIDC client
/// and HTTP client singleton across re-renders to avoid redundant provider discoveries.
pub fn use_oauth(config: OAuthConfig) -> (Signal<AuthState>, Callback<()>, Callback<()>) {
    let auth_state = use_signal(|| AuthState::Unauthenticated);
    let mut oidc_client = use_signal(|| None::<OpenIdClient>);
    let config_signal = use_signal(|| config);

    // Initialisierung & Callback Handling
    {
        let config_sig = config_signal.clone();
        let mut auth_state = auth_state;
        use_effect(move || {
            let params = parse_url_params();
            // Fehler-Handling
            if let Some(error) = params.get("error") {
                let error_description = params
                    .get("error_description")
                    .map(|d| d.replace('+', " "))
                    .unwrap_or_else(|| error.clone());
                auth_state.set(AuthState::Error(format!(
                    "OAuth Error: {}",
                    error_description
                )));
                clear_url();
                return;
            }

            // Async Block: Discovery + Callback oder Token Validierung
            spawn(async move {
                // Ensure OIDC Client is created / discovered (cached in signal)
                let client = match oidc_client.cloned() {
                    Some(c) => c,
                    None => {
                        let cfg = config_sig.read().clone();
                        match create_oidc_client(&cfg).await {
                            Ok(c) => {
                                oidc_client.set(Some(c.clone()));
                                c
                            }
                            Err(e) => {
                                auth_state.set(AuthState::Error(e));
                                return;
                            }
                        }
                    }
                };

                // Authorization Code Callback
                if let (Some(code), Some(state_returned)) =
                    (params.get("code"), params.get("state"))
                {
                    auth_state.set(AuthState::Authenticating);

                    // Check State
                    if let Some(expected_state) = get_stored_state() {
                        if &expected_state != state_returned {
                            auth_state.set(AuthState::Error("State mismatch".into()));
                            return;
                        }
                    } else {
                        auth_state.set(AuthState::Error("Missing stored state".into()));
                        return;
                    }

                    let code_verifier = match get_stored_code_verifier() {
                        Some(v) => PkceCodeVerifier::new(v),
                        None => {
                            auth_state.set(AuthState::Error("Missing PKCE verifier".into()));
                            return;
                        }
                    };

                    let auth_code = AuthorizationCode::new(code.clone());
                    let http = get_http_client();
                    let token_res = client
                        .exchange_code(auth_code)
                        .set_pkce_verifier(code_verifier)
                        .request_async(http)
                        .await;

                    match token_res {
                        Ok(token_response) => {
                            // Validate Nonce if ID Token present
                            if let Some(id_token) = token_response.extra_fields().id_token() {
                                match get_stored_nonce() {
                                    Some(nonce_str) => {
                                        let nonce = Nonce::new(nonce_str);
                                        if let Err(e) =
                                            id_token.claims(&client.id_token_verifier(), &nonce)
                                        {
                                            auth_state.set(AuthState::Error(format!(
                                                "ID token validation (nonce) failed: {e}"
                                            )));
                                            return;
                                        }
                                        remove_stored_nonce();
                                    }
                                    None => {
                                        auth_state.set(AuthState::Error(
                                            "Missing stored nonce".into(),
                                        ));
                                        return;
                                    }
                                }
                            }

                            remove_stored_code_verifier();
                            remove_stored_state();

                            let refresh_token = token_response
                                .refresh_token()
                                .map(|r| r.secret().to_string());

                            let maybe_userinfo = match client
                                .user_info(token_response.access_token().clone(), None)
                            {
                                Ok(req) => match req.request_async(http).await {
                                    Ok(claims) => Some(claims),
                                    Err(_) => None,
                                },
                                Err(_) => None,
                            };

                            let ui = build_user_info_from_openid(
                                &token_response,
                                maybe_userinfo,
                                refresh_token.clone(),
                            );
                            store_user_info(&ui);
                            auth_state.set(AuthState::Authenticated(ui.clone()));
                            clear_url();

                            if let (Some(rt), Some(exp)) =
                                (refresh_token.clone(), token_response.expires_in())
                            {
                                schedule_refresh(
                                    client.clone(),
                                    rt,
                                    exp.as_secs(),
                                    auth_state,
                                );
                            }
                        }
                        Err(e) => {
                            auth_state.set(AuthState::Error(format!("Token exchange failed: {e}")));
                        }
                    }
                } else if let Some(user_info) = get_stored_user_info() {
                    let ui = user_info;
                    let now = (Date::new_0().get_time() / 1000.0) as u64;
                    let mut exp_opt: Option<u64> = None;
                    if let Some(Ok(decoded)) = ui.decode_id_token() {
                        if let Some(exp) = decoded.claims.get("exp").and_then(|v| v.as_u64()) {
                            exp_opt = Some(exp);
                        }
                    }

                    let needs_refresh = match exp_opt {
                        Some(exp) => exp <= now.saturating_add(60),
                        None => false,
                    };

                    if needs_refresh {
                        if let Some(rt) = ui.refresh_token.clone() {
                            attempt_refresh(
                                client.clone(),
                                rt,
                                auth_state,
                            );
                        } else {
                            auth_state.set(AuthState::Unauthenticated);
                        }
                    } else {
                        if let Some(exp) = exp_opt {
                            let expires_in_secs = exp.saturating_sub(now);
                            store_user_info(&ui);
                            auth_state.set(AuthState::Authenticated(ui.clone()));
                            if let Some(rt) = ui.refresh_token.clone() {
                                schedule_refresh(
                                    client.clone(),
                                    rt,
                                    expires_in_secs,
                                    auth_state,
                                );
                            }
                        } else {
                            store_user_info(&ui);
                            auth_state.set(AuthState::Authenticated(ui));
                        }
                    }
                }
            });
        });
    }

    let login = {
        let config_sig = config_signal.clone();
        Callback::<()>::new(move |_| {
            if let Some(client) = oidc_client.cloned() {
                let cfg = config_sig.read();
                initiate_login(&client, &cfg);
            } else {
                warn!("OIDC client not ready yet");
            }
        })
    };

    let logout = Callback::<()>::new(move |_| {
        initiate_logout(auth_state);
    });

    (auth_state, login, logout)
}
