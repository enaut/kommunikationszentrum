use crate::config::OAuthConfig;
use crate::oauth::token_storage::{
    remove_stored_code_verifier, remove_stored_nonce, remove_stored_state, remove_stored_user_info,
    store_code_verifier, store_nonce, store_state, store_user_info,
};
use crate::oauth::{AuthState, UserInfo};
use dioxus::prelude::*;
use openidconnect::{
    AuthenticationFlow, ClientId, CsrfToken, EndpointMaybeSet, EndpointNotSet, EndpointSet,
    IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, RedirectUrl, RefreshToken, Scope,
    core::{
        CoreClient, CoreProviderMetadata, CoreResponseType, CoreTokenResponse, CoreUserInfoClaims,
    },
};
use reqwest::Client as HttpClient;
use std::collections::HashMap;
use std::sync::OnceLock;
use web_sys::window;

pub type OpenIdClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
    EndpointMaybeSet,
>;

/// Returns a lazily initialized static HTTP client with connection pooling and redirection policies.
pub fn get_http_client() -> &'static HttpClient {
    static HTTP_CLIENT: OnceLock<HttpClient> = OnceLock::new();
    HTTP_CLIENT.get_or_init(|| {
        #[allow(unused_mut)]
        let mut builder = HttpClient::builder();
        #[cfg(not(target_arch = "wasm32"))]
        {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }
        builder
            .build()
            .expect("failed to build reqwest HTTP client for OIDC")
    })
}

/// Discover OIDC provider metadata and create configured `OpenIdClient`.
pub async fn create_oidc_client(cfg: &OAuthConfig) -> Result<OpenIdClient, String> {
    let issuer =
        IssuerUrl::new(cfg.issuer_url.clone()).map_err(|e| format!("IssuerUrl error: {e}"))?;

    let http = get_http_client();
    let provider_metadata = CoreProviderMetadata::discover_async(issuer, http)
        .await
        .map_err(|e| format!("Discovery failed: {e}"))?;

    let client_id = ClientId::new(cfg.client_id.clone());
    let redirect_url = RedirectUrl::new(cfg.redirect_uri.clone())
        .map_err(|e| format!("RedirectUrl error: {e}"))?;

    let auth_ep = provider_metadata.authorization_endpoint().clone();
    let token_ep = provider_metadata
        .token_endpoint()
        .ok_or_else(|| "Provider metadata missing token_endpoint".to_string())?
        .clone();

    let client = CoreClient::from_provider_metadata(provider_metadata, client_id, None)
        .set_redirect_uri(redirect_url)
        .set_auth_uri(auth_ep)
        .set_token_uri(token_ep);

    Ok(client)
}

/// URL parameter parsing from window location href.
pub fn parse_url_params() -> HashMap<String, String> {
    let mut params = HashMap::new();

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

/// Clear query params from current browser URL via History API.
pub fn clear_url() {
    if let Some(window) = window() {
        if let Ok(history) = window.history() {
            let _ = history.replace_state_with_url(
                &web_sys::wasm_bindgen::JsValue::NULL,
                "",
                Some("/"),
            );
        }
    }
}

/// Helper to extract string or first string element in array from json value
fn extract_string_or_first_array(val: &serde_json::Value) -> Option<String> {
    match val {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if !trimmed.is_empty() && trimmed.contains('@') {
                Some(trimmed.to_string())
            } else {
                None
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(s) = extract_string_or_first_array(item) {
                    return Some(s);
                }
            }
            None
        }
        serde_json::Value::Object(map) => {
            for key in &["value", "email", "address"] {
                if let Some(item) = map.get(*key) {
                    if let Some(s) = extract_string_or_first_array(item) {
                        return Some(s);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract email address from claims JSON, supporting standard and provider-specific keys/formats
pub fn extract_email_from_claims(claims: &serde_json::Value) -> Option<String> {
    for key in &["email", "mail", "emails", "email_address"] {
        if let Some(val) = claims.get(*key) {
            if let Some(email) = extract_string_or_first_array(val) {
                return Some(email);
            }
        }
    }
    for key in &["upn", "preferred_username"] {
        if let Some(val) = claims.get(*key).and_then(|v| v.as_str()) {
            let trimmed = val.trim();
            if trimmed.contains('@') && !trimmed.contains(' ') {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Fetch userinfo endpoint response as raw JSON value if endpoint is configured.
pub async fn fetch_raw_userinfo(
    client: &OpenIdClient,
    access_token: &str,
) -> Option<serde_json::Value> {
    let http = get_http_client();
    let url = client.user_info_url()?;
    let resp = http
        .get(url.as_str())
        .bearer_auth(access_token)
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?;
    if resp.status().is_success() {
        resp.json::<serde_json::Value>().await.ok()
    } else {
        None
    }
}

/// Construct `UserInfo` struct from OpenID token response and optional userinfo claims.
pub fn build_user_info_from_openid(
    token_response: &CoreTokenResponse,
    claims: Option<CoreUserInfoClaims>,
    raw_userinfo: Option<serde_json::Value>,
    refresh_token: Option<String>,
) -> UserInfo {
    let access_token = token_response.access_token().secret().to_string();
    let id_token = token_response
        .extra_fields()
        .id_token()
        .map(|id| id.to_string());
    let mut username = String::new();
    let mut email = None;
    let mut sub = String::new();
    let mut given_name = None;
    let mut family_name = None;
    let mut name = None;

    if let Some(c) = &claims {
        if let Some(s) = c.preferred_username() {
            username = s.to_string();
        }
        if let Some(s) = c.email() {
            email = Some(s.to_string());
        }
        let sid = c.subject().as_str();
        sub = sid.to_string();
        if let Some(g) = c.given_name().and_then(|n| n.get(None)) {
            given_name = Some(g.to_string());
        }
        if let Some(f) = c.family_name().and_then(|n| n.get(None)) {
            family_name = Some(f.to_string());
        }
        if let Some(n) = c.name().and_then(|n| n.get(None)) {
            name = Some(n.to_string());
        }
    }

    if let Some(raw) = &raw_userinfo {
        if sub.is_empty() {
            if let Some(s) = raw.get("sub").and_then(|v| v.as_str()) {
                sub = s.to_string();
            }
        }
        if email.is_none() {
            email = extract_email_from_claims(raw);
        }
        if username.is_empty() {
            if let Some(u) = raw
                .get("preferred_username")
                .or_else(|| raw.get("username"))
                .or_else(|| raw.get("nickname"))
                .and_then(|v| v.as_str())
            {
                username = u.to_string();
            }
        }
        if name.is_none() {
            if let Some(n) = raw
                .get("name")
                .or_else(|| raw.get("display_name"))
                .and_then(|v| v.as_str())
            {
                name = Some(n.to_string());
            }
        }
        if given_name.is_none() {
            if let Some(g) = raw
                .get("given_name")
                .or_else(|| raw.get("first_name"))
                .and_then(|v| v.as_str())
            {
                given_name = Some(g.to_string());
            }
        }
        if family_name.is_none() {
            if let Some(f) = raw
                .get("family_name")
                .or_else(|| raw.get("last_name"))
                .and_then(|v| v.as_str())
            {
                family_name = Some(f.to_string());
            }
        }
    }

    if let Some(ref id_str) = id_token {
        if let Ok(decoded) = crate::oauth::jwt_utils::decode_jwt(id_str) {
            if sub.is_empty() {
                if let Some(s) = decoded.claims.get("sub").and_then(|v| v.as_str()) {
                    sub = s.to_string();
                }
            }
            if email.is_none() {
                email = extract_email_from_claims(&decoded.claims);
            }
            if username.is_empty() {
                if let Some(u) = decoded
                    .claims
                    .get("preferred_username")
                    .or_else(|| decoded.claims.get("username"))
                    .or_else(|| decoded.claims.get("nickname"))
                    .and_then(|v| v.as_str())
                {
                    username = u.to_string();
                }
            }
            if name.is_none() {
                if let Some(n) = decoded
                    .claims
                    .get("name")
                    .or_else(|| decoded.claims.get("display_name"))
                    .and_then(|v| v.as_str())
                {
                    name = Some(n.to_string());
                }
            }
            if given_name.is_none() {
                if let Some(g) = decoded
                    .claims
                    .get("given_name")
                    .or_else(|| decoded.claims.get("first_name"))
                    .and_then(|v| v.as_str())
                {
                    given_name = Some(g.to_string());
                }
            }
            if family_name.is_none() {
                if let Some(f) = decoded
                    .claims
                    .get("family_name")
                    .or_else(|| decoded.claims.get("last_name"))
                    .and_then(|v| v.as_str())
                {
                    family_name = Some(f.to_string());
                }
            }
        }
    }

    if username.is_empty() {
        username = sub.clone();
    }
    UserInfo {
        username,
        email,
        access_token,
        id_token,
        refresh_token,
        subject_id: sub,
        given_name,
        family_name,
        name,
    }
}

/// Plan automatic token refresh ~60s before expiry.
pub fn schedule_refresh(
    client: OpenIdClient,
    refresh_token: String,
    expires_in_secs: u64,
    auth_state: Signal<AuthState>,
) {
    let wait_ms = expires_in_secs.saturating_sub(60) * 1000; // 60s buffer
    if wait_ms == 0 {
        attempt_refresh(client, refresh_token, auth_state);
        return;
    }
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(wait_ms as u32).await;
        attempt_refresh(client, refresh_token, auth_state);
    });
}

/// Perform token refresh with the OpenID provider.
pub fn attempt_refresh(
    client: OpenIdClient,
    refresh_token: String,
    mut auth_state: Signal<AuthState>,
) {
    spawn(async move {
        let current_snapshot = auth_state.read().clone();
        if let AuthState::Authenticated(current) = current_snapshot {
            let rt = RefreshToken::new(refresh_token.clone());
            let http = get_http_client();
            match client.exchange_refresh_token(&rt).request_async(http).await {
                Ok(token_response) => {
                    let new_refresh = token_response
                        .refresh_token()
                        .map(|r| r.secret().to_string())
                        .or_else(|| Some(refresh_token.clone()));
                    let maybe_userinfo =
                        match client.user_info(token_response.access_token().clone(), None) {
                            Ok(req) => match req.request_async(http).await {
                                Ok(claims) => Some(claims),
                                Err(_) => None,
                            },
                            Err(_) => None,
                        };
                    let raw_userinfo = if maybe_userinfo.as_ref().and_then(|c| c.email()).is_none() {
                        fetch_raw_userinfo(&client, token_response.access_token().secret()).await
                    } else {
                        None
                    };
                    let mut updated =
                        build_user_info_from_openid(&token_response, maybe_userinfo, raw_userinfo, new_refresh);
                    if updated.name.is_none() {
                        updated.name = current.name.clone();
                    }
                    if updated.given_name.is_none() {
                        updated.given_name = current.given_name.clone();
                    }
                    if updated.family_name.is_none() {
                        updated.family_name = current.family_name.clone();
                    }
                    if updated.email.is_none() {
                        updated.email = current.email.clone();
                    }
                    store_user_info(&updated);
                    auth_state.set(AuthState::Authenticated(updated.clone()));
                    if let (Some(rt), Some(exp)) =
                        (updated.refresh_token.clone(), token_response.expires_in())
                    {
                        schedule_refresh(client, rt, exp.as_secs(), auth_state);
                    }
                }
                Err(e) => {
                    auth_state.set(AuthState::Error(format!("Refresh failed: {e}")));
                    remove_stored_user_info();
                }
            }
        }
    });
}

/// Initiate OAuth authorization code redirect.
pub fn initiate_login(client: &OpenIdClient, cfg: &OAuthConfig) {
    let mut auth_req = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".into()));
    for sc in cfg.scope.split_whitespace() {
        if sc != "openid" {
            auth_req = auth_req.add_scope(Scope::new(sc.into()));
        }
    }

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    store_code_verifier(pkce_verifier.secret());
    let (auth_url, csrf_token, nonce) = auth_req.set_pkce_challenge(pkce_challenge).url();
    store_state(csrf_token.secret());
    store_nonce(nonce.secret());
    if let Some(window) = window() {
        let _ = window.location().assign(auth_url.as_str());
    }
}

/// Clear stored tokens, credentials, and set auth state to unauthenticated.
pub fn initiate_logout(mut auth_state: Signal<AuthState>) {
    remove_stored_user_info();
    remove_stored_code_verifier();
    remove_stored_state();
    remove_stored_nonce();
    auth_state.set(AuthState::Unauthenticated);
}
