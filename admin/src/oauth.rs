use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use web_sys::window;
// openidconnect crate (4.x API – ohne stateless async_http_client Helper)
use openidconnect::{
    core::{
        CoreClient, CoreProviderMetadata, CoreResponseType, CoreTokenResponse, CoreUserInfoClaims,
    },
    AuthenticationFlow, AuthorizationCode, ClientId, CsrfToken, IssuerUrl, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope,
};
use openidconnect::{EndpointMaybeSet, EndpointNotSet, EndpointSet};
use reqwest::Client as HttpClient;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::warn;

// Wir verlassen uns auf openid_client für PKCE (es generiert Verifier & Challenge intern wenn nicht vorgegeben).

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
    pub issuer_url: String, // Discovery URL (Issuer base, ohne /.well-known/... => discover fügt an)
    pub client_id: String,  // Public SPA client id
    pub redirect_uri: String, // Registered redirect URI
    pub scope: String,      // Space separated scopes
    pub django_base_url: String, // Für rückwärtskompatible Felder (UserInfo Pfad etc.)
}

impl Default for OAuthConfig {
    fn default() -> Self {
        let django = "http://127.0.0.1:8000".to_string();
        Self {
            issuer_url: format!("{django}/o"), // Django OAuth Toolkit typischerweise unter /o/. Falls Discovery unter /.well-known/openid-configuration liegt, kann hier direkt Basis genutzt werden.
            client_id: "admin-app".to_string(),
            redirect_uri: "http://127.0.0.1:8080/callback".to_string(),
            scope: "openid profile email".to_string(),
            django_base_url: django,
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

pub fn use_oauth() -> (Signal<AuthState>, Callback<()>, Callback<()>) {
    let auth_state = use_signal(|| AuthState::Unauthenticated);
    let config = use_signal(OAuthConfig::default);

    // OIDC Client (lazy). Kein expliziter Typ für die Endpoint-Typzustände.
    let oidc_client: Rc<RefCell<Option<_>>> = Rc::new(RefCell::new(None));
    // Persistenter HTTP Client (AsyncHttpClient Trait). Redirects (SSRF Schutz) nur deaktivieren außerhalb WASM.
    let http_client = Rc::new({
        #[allow(unused_mut)]
        let mut builder = HttpClient::builder();
        #[cfg(not(target_arch = "wasm32"))]
        {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }
        builder
            .build()
            .expect("failed to build reqwest HTTP client for OIDC")
    });

    // Initialisierung & Callback Handling
    {
        let oidc_client = oidc_client.clone();
        let http_client_discovery = http_client.clone();
        let mut auth_state = auth_state;
        let config_sig = config.clone();
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
            let oidc_client_outer = oidc_client.clone();
            let http_client_discovery_clone = http_client_discovery.clone();
            let http_client_discovery_for_refresh = http_client_discovery.clone();
            let http_client_discovery_inner = http_client_discovery.clone();
            spawn(async move {
                // Discovery (wenn noch kein Client)
                if oidc_client_outer.borrow().is_none() {
                    let cfg = config_sig.read().clone();
                    let issuer = match IssuerUrl::new(cfg.issuer_url.clone()) {
                        Ok(i) => i,
                        Err(e) => {
                            auth_state.set(AuthState::Error(format!("IssuerUrl error: {e}")));
                            return;
                        }
                    };
                    let provider_metadata = match CoreProviderMetadata::discover_async(
                        issuer,
                        &*http_client_discovery_clone,
                    )
                    .await
                    {
                        Ok(m) => m,
                        Err(e) => {
                            auth_state.set(AuthState::Error(format!("Discovery failed: {e}")));
                            return;
                        }
                    };
                    let client_id = ClientId::new(cfg.client_id.clone());
                    let redirect_url = match RedirectUrl::new(cfg.redirect_uri.clone()) {
                        Ok(r) => r,
                        Err(e) => {
                            auth_state.set(AuthState::Error(format!("RedirectUrl error: {e}")));
                            return;
                        }
                    };
                    // Endpoints setzen (Auth & Token obligatorisch für Authorization Code Flow)
                    let auth_ep = provider_metadata.authorization_endpoint().clone();
                    let token_ep = match provider_metadata.token_endpoint() {
                        Some(t) => t.clone(),
                        None => {
                            auth_state.set(AuthState::Error(
                                "Provider metadata missing token_endpoint".into(),
                            ));
                            return;
                        }
                    };
                    let client =
                        CoreClient::from_provider_metadata(provider_metadata, client_id, None)
                            .set_redirect_uri(redirect_url)
                            .set_auth_uri(auth_ep)
                            .set_token_uri(token_ep);
                    // UserInfo Endpoint bleibt MaybeSet typzustand (wir setzen ihn nicht explizit, verwenden user_info_maybe fallibel)
                    oidc_client_outer.replace(Some(client));
                }

                // Authorization Code Callback
                if let (Some(code), Some(state_returned)) =
                    (params.get("code"), params.get("state"))
                {
                    auth_state.set(AuthState::Authenticating);
                    if let Some(client) = oidc_client_outer.borrow().as_ref() {
                        // Prüfe State
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
                        // Token Request
                        let token_res = client
                            .exchange_code(auth_code)
                            .set_pkce_verifier(code_verifier)
                            .request_async(&*http_client_discovery_inner)
                            .await;
                        match token_res {
                            Ok(token_response) => {
                                // Nonce validieren (falls ID Token vorhanden)
                                if let Some(id_token) = token_response.extra_fields().id_token() {
                                    match get_stored_nonce() {
                                        Some(nonce_str) => {
                                            let nonce = Nonce::new(nonce_str.clone());
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
                                // Refresh Token extrahieren (falls vorhanden) bevor move
                                let refresh_token = token_response
                                    .refresh_token()
                                    .map(|r| r.secret().to_string());
                                // UserInfo abrufen (optional) – wenn Endpoint vorhanden
                                let maybe_userinfo = match client
                                    .user_info(token_response.access_token().clone(), None)
                                {
                                    Ok(req) => {
                                        match req.request_async(&*http_client_discovery_inner).await
                                        {
                                            Ok(claims) => Some(claims),
                                            Err(_) => None,
                                        }
                                    }
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
                                        auth_state.clone(),
                                        http_client_discovery_for_refresh.clone(),
                                    );
                                }
                            }
                            Err(e) => auth_state
                                .set(AuthState::Error(format!("Token exchange failed: {e}"))),
                        }
                    }
                } else if let Some(user_info) = get_stored_user_info() {
                    auth_state.set(AuthState::Authenticated(user_info));
                }
            });
        });
    }

    // Login Callback => generiert Auth URL via Client::authorization_url
    let login = {
        let config_sig = config.clone();
        let oidc_client = oidc_client.clone();
        Callback::<()>::new(move |_| {
            if let Some(client) = oidc_client.borrow().as_ref() {
                let cfg = config_sig.read();
                // Scopes
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
                // PKCE
                let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
                store_code_verifier(pkce_verifier.secret());
                let (auth_url, csrf_token, nonce) =
                    auth_req.set_pkce_challenge(pkce_challenge).url();
                store_state(csrf_token.secret());
                store_nonce(nonce.secret());
                if let Some(window) = window() {
                    let _ = window.location().assign(auth_url.as_str());
                }
            } else {
                warn!("OIDC client not ready yet");
            }
        })
    };

    let logout = {
        let mut auth_state = auth_state;
        Callback::<()>::new(move |_| {
            remove_stored_user_info();
            remove_stored_code_verifier();
            remove_stored_state();
            remove_stored_nonce();
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
    window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("oauth_code_verifier").ok().flatten())
}
fn store_code_verifier(code_verifier: &str) {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.set_item("oauth_code_verifier", code_verifier);
    }
}
fn remove_stored_code_verifier() {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.remove_item("oauth_code_verifier");
    }
}

fn clear_url() {
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

fn build_user_info_from_openid(
    token_response: &CoreTokenResponse,
    claims: Option<CoreUserInfoClaims>,
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
    let groups: Option<Vec<String>> = None;
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
    if username.is_empty() {
        username = sub.clone();
    }
    UserInfo {
        username,
        email,
        access_token,
        id_token,
        refresh_token,
        mitgliedsnr: sub,
        given_name,
        family_name,
        name,
        is_staff: None,
        is_superuser: None,
        groups,
    }
}

// Planung einer automatischen Token-Erneuerung ~60s vor Ablauf
fn schedule_refresh(
    client: CoreClient<
        EndpointSet,
        EndpointNotSet,
        EndpointNotSet,
        EndpointNotSet,
        EndpointSet,
        EndpointMaybeSet,
    >,
    refresh_token: String,
    expires_in_secs: u64,
    auth_state: Signal<AuthState>,
    http_client: Rc<HttpClient>,
) {
    let wait_ms = expires_in_secs.saturating_sub(60) * 1000; // 60s Puffer
    if wait_ms == 0 {
        attempt_refresh(client, refresh_token, auth_state, http_client);
        return;
    }
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(wait_ms as u32).await;
        attempt_refresh(client, refresh_token, auth_state, http_client);
    });
}

fn attempt_refresh(
    client: CoreClient<
        EndpointSet,
        EndpointNotSet,
        EndpointNotSet,
        EndpointNotSet,
        EndpointSet,
        EndpointMaybeSet,
    >,
    refresh_token: String,
    auth_state: Signal<AuthState>,
    http_client: Rc<HttpClient>,
) {
    let mut auth_state_cloned = auth_state.clone();
    spawn(async move {
        // Snapshot des aktuellen Zustands ohne Borrow während async weiterer Mutationen
        let current_snapshot = auth_state_cloned.read().clone();
        if let AuthState::Authenticated(current) = current_snapshot {
            let rt = RefreshToken::new(refresh_token.clone());
            match client
                .exchange_refresh_token(&rt)
                .request_async(&*http_client)
                .await
            {
                Ok(token_response) => {
                    let new_refresh = token_response
                        .refresh_token()
                        .map(|r| r.secret().to_string())
                        .or_else(|| Some(refresh_token.clone()));
                    let maybe_userinfo =
                        match client.user_info(token_response.access_token().clone(), None) {
                            Ok(req) => match req.request_async(&*http_client).await {
                                Ok(claims) => Some(claims),
                                Err(_) => None,
                            },
                            Err(_) => None,
                        };
                    let mut updated =
                        build_user_info_from_openid(&token_response, maybe_userinfo, new_refresh);
                    // Fehlende Felder aus vorherigem Zustand übernehmen
                    if updated.name.is_none() {
                        updated.name = current.name.clone();
                    }
                    if updated.given_name.is_none() {
                        updated.given_name = current.given_name.clone();
                    }
                    if updated.family_name.is_none() {
                        updated.family_name = current.family_name.clone();
                    }
                    store_user_info(&updated);
                    auth_state_cloned.set(AuthState::Authenticated(updated.clone()));
                    if let (Some(rt), Some(exp)) =
                        (updated.refresh_token.clone(), token_response.expires_in())
                    {
                        schedule_refresh(
                            client.clone(),
                            rt,
                            exp.as_secs(),
                            auth_state_cloned.clone(),
                            http_client.clone(),
                        );
                    }
                }
                Err(e) => {
                    auth_state_cloned.set(AuthState::Error(format!("Refresh failed: {e}")));
                    remove_stored_user_info();
                }
            }
        }
    });
}

// --- State & Nonce Speicherung
fn store_state(state: &str) {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.set_item("oauth_state", state);
    }
}
fn get_stored_state() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("oauth_state").ok().flatten())
}
fn remove_stored_state() {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.remove_item("oauth_state");
    }
}
fn store_nonce(nonce: &str) {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.set_item("oauth_nonce", nonce);
    }
}
fn get_stored_nonce() -> Option<String> {
    window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("oauth_nonce").ok().flatten())
}
fn remove_stored_nonce() {
    if let Some(s) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.remove_item("oauth_nonce");
    }
}
