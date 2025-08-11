# OAuth Integration

The OAuth integration connects the admin interface with the Django solawispielplatz system for user authentication. The implementation uses OAuth 2.0 Authorization Code Flow with PKCE (Proof Key for Code Exchange) plus OpenID Connect to obtain an ID token (JWT) and user info claims.

## Configuration

### Django OAuth Provider

The Django system acts as the OAuth 2.0 authorization server with these settings:

**Base Configuration**:
- **Issuer URL**: `http://127.0.0.1:8000/o`
- **Client ID**: `admin-app` (configured as public client)
- **Authorization Endpoint**: `/o/authorize/`
- **Token Endpoint**: `/o/token/`

**Security Features**:
- **PKCE Required**: Prevents authorization code interception
- **OIDC Enabled**: Provides JWT ID tokens with user claims
- **Public Client**: No client secret required (suitable for frontend applications)

Required Django settings in `settings_local.py`:

```python
OAUTH2_PROVIDER = {
    "OIDC_ENABLED": True,
    "PKCE_REQUIRED": True,
    "OIDC_ISS_ENDPOINT": "http://127.0.0.1:8000/o",
    "OAUTH2_VALIDATOR_CLASS": "authentifizierung.oauth_validator.CustomOAuth2Validator",
}
```

### Admin Interface Configuration

The Rust admin interface configures OAuth through the `OAuthConfig` structure:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,           // "admin-app"
    pub redirect_uri: String,        // Local redirect URL
    pub django_base_url: String,     // Django server base URL
}
```

Default configuration connects to local Django instance:
- **Client ID**: `admin-app`
- **Redirect URI**: `http://localhost:8080/callback`
- **Django Base**: `http://127.0.0.1:8000`

## OAuth Flow Implementation

### Authorization Request

The admin interface builds the authorization URL with the `openidconnect` crate after dynamic discovery:

```rust
let (auth_url, state, nonce) = client
    .authorize_url(AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                   CsrfToken::new_random,
                   Nonce::new_random)
    .add_scope(Scope::new("openid".into()))
    .add_scope(Scope::new("profile".into()))
    .add_scope(Scope::new("email".into()))
    .set_pkce_challenge(pkce_challenge)
    .url();

// state.secret() and nonce.secret() are persisted in localStorage.
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
let token_response = client
    .exchange_code(auth_code)
    .set_pkce_verifier(code_verifier)
    .request_async(async_http_client)
    .await?;

if let Some(id_token) = token_response.extra_fields().id_token() {
    let nonce = Nonce::new(stored_nonce);
    id_token.claims(&client.id_token_verifier(), &nonce)?; // validates signature, aud, iss, exp, nonce
}
```

On nonce validation success the stored nonce is removed; failure aborts authentication.

### User Information Retrieval

The `openidconnect` client attempts a `userinfo` request if the discovery metadata advertises an endpoint. Claims (preferred_username, email, given_name, etc.) are merged into the internal `UserInfo` structure together with the subject (`sub`).

## Token & State Storage

Stored keys:
- `oauth_user_info`: Serialized `UserInfo` (access token, optional ID token, basic claims)
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

All errors clear sensitive stored values and place the system back into an unauthenticated state.
