# OAuth / OpenID Connect Integration

The authentication integration connects the admin interface with any standard OpenID Connect (OIDC) provider (such as Nextcloud, Keycloak, Authentik, or Django) for user authentication. The implementation uses OAuth 2.0 Authorization Code Flow with PKCE (Proof Key for Code Exchange) plus OpenID Connect to obtain an ID token (JWT) and user info claims.

## Configuration

### OIDC Provider Requirements

The provider must support standard OpenID Connect discovery and authorization code flow with PKCE:

**Base Requirements**:
- **Discovery Endpoint**: `/.well-known/openid-configuration`
- **JWKS Endpoint**: Defined in discovery metadata
- **Authorization Endpoint**: Supports `response_type=code` with PKCE (S256)
- **Token Endpoint**: Exchanges authorization code and refresh token
- **UserInfo Endpoint**: Returns standard claims (`sub`, `email`, `name`, `preferred_username`)

**Security Features**:
- **PKCE Required**: Enforced for public single-page clients
- **OIDC Enabled**: Provides RS256 JWT ID tokens with user claims
- **Public Client**: No client secret required in the browser frontend

### Admin Interface Configuration

The Rust admin interface configures OIDC through the `OAuthConfig` structure:

```rust
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub issuer_url: String,   // OIDC issuer URL (discovery base)
    pub client_id: String,    // "admin-app"
    pub redirect_uri: String, // Callback URL
    pub scope: String,        // "openid profile email"
}
```

Configuration is loaded from environment variables:
- `OIDC_ISSUER_URL`: e.g. `http://127.0.0.1:8000/o` or `https://cloud.example.org`
- `OIDC_CLIENT_ID`: default `"admin-app"`
- `ADMIN_REDIRECT_URI`: default `"http://127.0.0.1:8080/callback"`
- `OAUTH_SCOPES`: default `"openid profile email"`

## OAuth Flow Implementation

### Authorization Request

The admin interface builds the authorization URL with the `openidconnect` crate (v4) after dynamic discovery.

```rust
// Reusable HTTP client (no redirects to avoid SSRF)
let http = reqwest::Client::builder()
    .redirect(reqwest::redirect::Policy::none())
    .build()?;

// After discovery & client construction (auth & token endpoints set):
let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
store_code_verifier(pkce_verifier.secret());

let (auth_url, state, nonce) = client
    .authorize_url(
        AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
        CsrfToken::new_random,
        Nonce::new_random,
    )
    .add_scope(Scope::new("openid".into()))
    .add_scope(Scope::new("profile".into()))
    .add_scope(Scope::new("email".into()))
    .set_pkce_challenge(pkce_challenge)
    .url();

store_state(state.secret());
store_nonce(nonce.secret());
// Redirect the browser to auth_url
```

Stored values in `localStorage`:
- `oauth_state`: CSRF protection value validated on callback
- `oauth_code_verifier`: PKCE verifier used for token exchange
- `oauth_nonce`: Nonce validated against the ID token to prevent token replay

### Token Exchange & Validation

After redirect back with `code` and `state` parameters:

1. Validate `state` equals stored `oauth_state`.
2. Retrieve PKCE verifier and call `exchange_code(..).set_pkce_verifier(..)`.
3. On successful response remove `oauth_state` and `oauth_code_verifier`.
4. If an ID token is present, validate its claims including the stored nonce:

```rust
// Exchange the authorization code using the same persistent reqwest client
let token_response = client
    .exchange_code(auth_code)
    .set_pkce_verifier(code_verifier)
    .request_async(&http)
    .await?;

if let Some(id_token) = token_response.extra_fields().id_token() {
    let nonce = Nonce::new(stored_nonce);
    id_token.claims(&client.id_token_verifier(), &nonce)?; // signature, aud, iss, exp, nonce
    remove_stored_nonce();
}
```

On nonce validation success the stored nonce is removed; failure aborts authentication.

If the token response includes a refresh token (Django configured to issue one for public clients), it is stored inside the serialized `oauth_user_info` object for silent renewal.

### User Information Retrieval

The `openidconnect` client attempts a `userinfo` request only if the discovery metadata included a user info endpoint (typestate `EndpointMaybeSet`). The call is fallible; a missing endpoint or network error is ignored gracefully:

```rust
let maybe_userinfo = client
    .user_info(token_response.access_token().clone(), None)
    .ok()
    .and_then(|req| async {
        req.request_async(&http).await.ok()
    }.await);

// Merge optional claims into internal UserInfo structure.
```

## Token & State Storage

Stored keys:
- `oauth_user_info`: Serialized `UserInfo` (access token, optional ID token, basic claims)
- `refresh_token` (inside `oauth_user_info`): Used for silent renewal shortly before expiry
- `oauth_state`: CSRF state (removed after callback)
- `oauth_code_verifier`: PKCE verifier (removed after token exchange)
- `oauth_nonce`: Nonce for ID token validation (removed after validation)

Security notes:
- No client secret is embedded (public SPA).
- State, code verifier and nonce are single-use and removed after success to reduce replay surface.

## Error Handling

Main handled error classes:
- Authorization errors (error / error_description in callback query).
- State mismatch / missing state.
- Missing PKCE verifier or nonce.
- Token exchange failures.
- ID token / nonce validation failures.
- Refresh failures (refresh token invalid / expired) trigger a forced logout.

All errors clear sensitive stored values and place the system back into an unauthenticated state.
