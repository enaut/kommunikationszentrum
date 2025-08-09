# OAuth Integration

The OAuth integration connects the admin interface with the Django solawispielplatz system for user authentication. This implementation uses OAuth 2.0 with PKCE (Proof Key for Code Exchange) and OpenID Connect for secure token-based authentication.

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

The admin interface initiates OAuth flow using the `dioxus_oauth` crate:

```rust
let client = dioxus_oauth::prelude::OAuthClient::new(
    &config.client_id,
    &config.redirect_uri,
    &format!("{}/o/authorize/", config.django_base_url),
    &format!("{}/o/token/", config.django_base_url),
);
```

The authorization request includes:
- **Response Type**: `code` (authorization code flow)
- **PKCE Challenge**: SHA256 hash of code verifier
- **Scope**: `openid profile email` for OIDC claims

### Token Exchange

After receiving the authorization code, the client exchanges it for tokens:

```rust
let token_response = client.get_token(&code).await?;
```

The token response contains:
- **Access Token**: For API access to Django
- **ID Token**: JWT for SpacetimeDB authentication
- **Refresh Token**: For token renewal (if configured)
- **Token Type**: `Bearer`
- **Expires In**: Token lifetime in seconds

### User Information Retrieval

The access token is used to fetch user information from Django's user info endpoint:

```rust
let user_response = get_user_info(&token_response.access_token, &config.django_base_url).await?;
```

User information includes:
- Username and email address
- First and last name
- Django user ID (subject claim)
- Staff and superuser status
- Group memberships

## Token Storage

The admin interface manages token persistence:

**Local Storage**: Access tokens are stored in browser's localStorage for session persistence across page reloads.

**Security Considerations**: Tokens are stored client-side only and automatically cleared on logout or authentication errors.

**Session Management**: The interface checks for stored tokens on startup and validates them before establishing authenticated connections.

## Error Handling

The OAuth implementation handles various error conditions:

**Authorization Errors**: Invalid client configuration, user denial, or server errors during authorization.

**Token Exchange Errors**: Network failures, invalid authorization codes, or server-side token generation issues.

**Validation Errors**: Expired or invalid stored tokens trigger re-authentication flows.

Each error type provides specific user feedback and appropriate recovery actions, such as retry mechanisms or fallback to login screen.
