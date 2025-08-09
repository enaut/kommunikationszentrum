use base64::{engine::general_purpose, Engine as _};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use web_sys::window;

// PKCE helper functions
fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("Failed to generate random bytes");
    general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn generate_code_challenge(code_verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let result = hasher.finalize();
    general_purpose::URL_SAFE_NO_PAD.encode(result)
}

// URL parameter parsing
fn parse_url_params() -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();

    if let Some(window) = window() {
        if let Ok(url) = window.location().href() {
            if let Some(query_start) = url.find('?') {
                let query = &url[query_start + 1..];
                for pair in query.split('&') {
                    if let Some((key, value)) = pair.split_once('=') {
                        params.insert(
                            urlencoding::decode(key).unwrap_or_default().to_string(),
                            urlencoding::decode(value).unwrap_or_default().to_string(),
                        );
                    }
                }
            }
        }
    }

    params
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub django_base_url: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            // You'll need to update this to match your Django server
            django_base_url: "http://127.0.0.1:8000".to_string(),
            client_id: "admin-app".to_string(), // This needs to be configured in Django OAuth2 settings
            redirect_uri: "http://127.0.0.1:8080/callback".to_string(),
            scope: "openid profile email".to_string(), // Request additional scopes
        }
    }
}

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
    pub id_token: Option<String>, // JWT for SpacetimeDB auth
    pub mitgliedsnr: String,      // Subject from JWT (Mitgliedsnummer)
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub name: Option<String>,
    pub is_staff: Option<bool>,
    pub is_superuser: Option<bool>,
    pub groups: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
    scope: String,
    id_token: Option<String>, // JWT ID token from OIDC
}

#[derive(Debug, Serialize, Deserialize)]
struct UserInfoResponse {
    sub: String,                // User ID from Django
    preferred_username: String, // Username
    email: Option<String>,
    email_verified: Option<bool>,
    given_name: Option<String>,
    family_name: Option<String>,
    name: Option<String>,
    is_staff: Option<bool>,
    is_superuser: Option<bool>,
    groups: Option<Vec<String>>,
}

pub fn use_oauth() -> (Signal<AuthState>, Callback<()>, Callback<()>) {
    let mut auth_state = use_signal(|| AuthState::Unauthenticated);
    let config = use_signal(OAuthConfig::default);

    // Check for OAuth callback or stored token on mount
    use_effect(move || {
        let params = parse_url_params();

        // Check for OAuth error in URL
        if let Some(error) = params.get("error") {
            let error_description = params
                .get("error_description")
                .map(|d| d.replace('+', " "))
                .unwrap_or_else(|| error.clone());

            auth_state.set(AuthState::Error(format!(
                "OAuth Error: {}",
                error_description
            )));

            // Clear the URL
            if let Some(window) = window() {
                if let Ok(history) = window.history() {
                    let _ = history.replace_state_with_url(
                        &web_sys::wasm_bindgen::JsValue::NULL,
                        "",
                        Some("/"),
                    );
                }
            }
            return;
        }

        // Check for OAuth success callback
        if let Some(code) = params.get("code") {
            auth_state.set(AuthState::Authenticating);

            let code = code.clone();
            let config = config.read().clone();
            let mut auth_state = auth_state;

            spawn(async move {
                // Get stored code verifier
                let code_verifier = get_stored_code_verifier();

                if let Some(code_verifier) = code_verifier {
                    match exchange_code_for_token_with_pkce(code, config, code_verifier).await {
                        Ok(user_info) => {
                            // Clear code verifier
                            remove_stored_code_verifier();
                            store_user_info(&user_info);
                            auth_state.set(AuthState::Authenticated(user_info));
                        }
                        Err(e) => {
                            auth_state
                                .set(AuthState::Error(format!("Token exchange failed: {}", e)));
                        }
                    }
                } else {
                    auth_state.set(AuthState::Error("Missing code verifier".to_string()));
                }

                // Clear the URL
                if let Some(window) = window() {
                    if let Ok(history) = window.history() {
                        let _ = history.replace_state_with_url(
                            &web_sys::wasm_bindgen::JsValue::NULL,
                            "",
                            Some("/"),
                        );
                    }
                }
            });
            return;
        }

        // Check for stored user info
        if let Some(user_info) = get_stored_user_info() {
            auth_state.set(AuthState::Authenticating);

            let mut auth_state = auth_state;
            let user_info_clone = user_info.clone();

            spawn(async move {
                match validate_token(
                    user_info.access_token.clone(),
                    config.read().django_base_url.clone(),
                )
                .await
                {
                    Ok(_) => {
                        auth_state.set(AuthState::Authenticated(user_info_clone));
                    }
                    Err(_) => {
                        // Token invalid, remove it
                        remove_stored_user_info();
                        auth_state.set(AuthState::Unauthenticated);
                    }
                }
            });
        }
    });

    let login = {
        Callback::<()>::new(move |_| {
            // Generate PKCE parameters
            let code_verifier = generate_code_verifier();
            let code_challenge = generate_code_challenge(&code_verifier);

            // Store code verifier
            store_code_verifier(&code_verifier);

            let config = config.read();
            let auth_url = format!(
                "{}/o/authorize/?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256",
                config.django_base_url,
                urlencoding::encode(&config.client_id),
                urlencoding::encode(&config.redirect_uri),
                urlencoding::encode(&config.scope),
                urlencoding::encode(&code_challenge)
            );

            if let Some(window) = window() {
                let _ = window.location().assign(&auth_url);
            }
        })
    };

    let logout = {
        let mut auth_state = auth_state;
        Callback::<()>::new(move |_| {
            remove_stored_user_info();
            remove_stored_code_verifier();
            auth_state.set(AuthState::Unauthenticated);
        })
    };

    (auth_state, login, logout)
}

fn get_stored_user_info() -> Option<UserInfo> {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        if let Ok(Some(user_info_str)) = storage.get_item("oauth_user_info") {
            return serde_json::from_str(&user_info_str).ok();
        }
    }
    None
}

fn store_user_info(user_info: &UserInfo) {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        if let Ok(user_info_str) = serde_json::to_string(user_info) {
            let _ = storage.set_item("oauth_user_info", &user_info_str);
        }
    }
}

fn remove_stored_user_info() {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.remove_item("oauth_user_info");
    }
}

fn get_stored_code_verifier() -> Option<String> {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        return storage.get_item("oauth_code_verifier").ok().flatten();
    }
    None
}

fn store_code_verifier(code_verifier: &str) {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.set_item("oauth_code_verifier", code_verifier);
    }
}

fn remove_stored_code_verifier() {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.remove_item("oauth_code_verifier");
    }
}

// Legacy token storage functions (kept for backward compatibility)
async fn exchange_code_for_token_with_pkce(
    code: String,
    config: OAuthConfig,
    code_verifier: String,
) -> Result<UserInfo, String> {
    let client = reqwest::Client::new();

    // Exchange authorization code for access token with PKCE
    let token_url = format!("{}/o/token/", config.django_base_url);

    let params = [
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", &config.redirect_uri),
        ("client_id", &config.client_id),
        ("code_verifier", &code_verifier),
    ];

    let token_response = client
        .post(&token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;

    if !token_response.status().is_success() {
        let status = token_response.status();
        let error_text = token_response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!(
            "Token request failed with status {}: {}",
            status, error_text
        ));
    }

    let token_data: TokenResponse = token_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    // Get user information
    let user_info = get_user_info(&token_data.access_token, &config.django_base_url).await?;

    Ok(UserInfo {
        username: user_info.preferred_username, // Use preferred_username from OIDC
        email: user_info.email,
        access_token: token_data.access_token,
        id_token: token_data.id_token, // JWT for SpacetimeDB
        mitgliedsnr: user_info.sub,    // Subject = Mitgliedsnummer
        given_name: user_info.given_name,
        family_name: user_info.family_name,
        name: user_info.name,
        is_staff: user_info.is_staff,
        is_superuser: user_info.is_superuser,
        groups: user_info.groups,
    })
}

async fn validate_token(
    token: String,
    django_base_url: String,
) -> Result<UserInfoResponse, String> {
    get_user_info(&token, &django_base_url).await
}

async fn get_user_info(token: &str, django_base_url: &str) -> Result<UserInfoResponse, String> {
    let client = reqwest::Client::new();
    let user_info_url = format!("{}/o/userinfo/", django_base_url);

    let response = client
        .get(&user_info_url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("User info request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!(
            "User info request failed with status {}: {}",
            status, error_text
        ));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse user info response: {}", e))
}
