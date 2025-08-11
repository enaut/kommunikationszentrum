# JWT Token Handling

JWT (JSON Web Token) tokens serve as the primary authentication mechanism between the admin interface and SpacetimeDB. The system uses OpenID Connect ID tokens that contain user identity and authorization claims from Django.

## Token Structure

### ID Token Claims

The JWT ID token issued by Django contains these standard and custom claims:

**Standard OIDC Claims**:
- `sub`: Subject identifier (Django user primary key)
- `iss`: Issuer URL (`http://127.0.0.1:8000/o`)
- `aud`: Audience (`admin-app`)
- `exp`: Expiration timestamp
- `iat`: Issued at timestamp
- `preferred_username`: Django username

**Profile Claims**:
- `email`: User email address
- `email_verified`: Email verification status
- `given_name`: First name
- `family_name`: Last name
- `name`: Full name

**Authorization Claims**:
- `is_staff`: Django staff member flag
- `is_superuser`: Django superuser flag
- `groups`: Array of Django group names

### Token Validation

SpacetimeDB automatically validates JWT tokens when clients connect:

**Cryptographic Verification**: Token signature is validated against Django's public key to ensure authenticity and integrity.

**Issuer Validation**: The `iss` claim must match the configured Django issuer URL.

**Audience Validation**: The `aud` claim must match the application identifier (`admin-app`).

**Expiration Check**: Current time must be before the `exp` timestamp.

**Nonce Validation**: The nonce stored during authorization must match the nonce claim in the ID token (validated via `openidconnect` library) to mitigate replay.

## SpacetimeDB Integration

### Connection Authentication

The admin interface provides the JWT ID token when connecting to SpacetimeDB:

```rust
let spacetime_db = use_spacetime_db(SpacetimeDbOptions {
    uri: "http://localhost:3000".to_string(),
    module_name: "kommunikation".to_string(),
    token: user_info.id_token.clone(),
});
```

SpacetimeDB configuration enables JWT authentication:

```toml
[auth]
enabled = true

[[auth.providers]]
issuer = "http://127.0.0.1:8000/o"
audience = "admin-app"
```

### Identity Context

Once authenticated, SpacetimeDB provides the verified identity to reducers through the `ReducerContext`:

```rust
#[spacetimedb::reducer]
pub fn authenticated_operation(ctx: &ReducerContext) -> Result<(), String> {
    // ctx.sender contains the authenticated Identity
    // derived from validated JWT claims
    log::info!("Operation requested by identity: {:?}", ctx.sender);
    Ok(())
}
```

## Token Lifecycle

### Acquisition

JWT ID tokens are obtained during the OAuth authorization flow:

1. User completes OAuth login with Django
2. Authorization code exchanged for token response
3. Token response contains both access token and ID token
4. ID token extracted for SpacetimeDB authentication

### Storage and Reuse

**Client-side Storage**: ID tokens are stored alongside access tokens in browser localStorage for session persistence.

**Connection Reuse**: Stored tokens are automatically used for SpacetimeDB connections on page reload or navigation.

**Validation on Use**: Stored tokens are validated before establishing new connections to ensure they haven't expired.

### Expiration Handling

**Automatic Detection**: SpacetimeDB rejects expired tokens, triggering re-authentication flows in the admin interface.

**User Experience**: Token expiration results in logout and redirect to login screen.

**Token Refresh**: Current implementation requires complete re-authentication; automatic refresh could be implemented using refresh tokens.

