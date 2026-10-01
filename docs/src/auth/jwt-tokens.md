# JWT Token Handling

JWT (JSON Web Token) tokens are the authentication mechanism between the admin interface and SpacetimeDB. The admin obtains an OpenID Connect ID token from the issuer in `OIDC_ISSUER_URL` and sends that token when it connects. SpacetimeDB standalone does not have a configured list of external issuers.

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

The admin client and SpacetimeDB validate the ID token separately.

The admin callback uses the `openidconnect` crate. That check covers the signature, issuer, audience, expiry, and the nonce stored for the authorization request.

SpacetimeDB standalone does not read an issuer allowlist. There is no `[auth]` section and no `[[auth.providers]]` array. The standalone `config.toml` in the server data directory accepts `certificate-authority`, `logs`, `module-http`, `wasm`, `v8`, `v8-heap-policy`, `commitlog`, and `websocket`. Other keys are ignored. `[certificate-authority]` `jwt-priv-key-path` and `jwt-pub-key-path`, and the matching `spacetime start` flags, are the server's own identity keys. They are not an external OIDC JWKS.

On connect, SpacetimeDB 2.7 and 2.10 validate a bearer token as follows:

1. A token signed by the server's own key is accepted. That path does not check the issuer.
2. Otherwise the unverified `iss` claim is used only to find keys. The server requests `{iss}/.well-known/openid-configuration`, follows redirects, and reads `jwks_uri`. A trailing slash on `iss` is removed. The URL scheme must be `http` or `https`.
3. The signature is checked against that JWKS. `RS256`, `ES256`, and `HS256` keys are accepted.
4. The verified `iss` must equal the issuer used for discovery. `sub` and `iss` are required. If `exp` is present, it must not be more than 60 seconds in the past.
5. `aud` is not checked. `OIDC_CLIENT_ID` is read only by the admin build.

Any issuer that publishes discovery and a JWKS which verifies the token is accepted.

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

### Identity Context

A validated external token becomes the connection identity `Identity::from_claims(iss, sub)`. Reducers see that value as `ctx.sender`.

```rust
#[spacetimedb::reducer]
pub fn authenticated_operation(ctx: &ReducerContext) -> Result<(), String> {
    // ctx.sender is Identity::from_claims(iss, sub) from the validated token.
    log::info!("Operation requested by identity: {:?}", ctx.sender);
    Ok(())
}
```

The module does not use the token issuer when it stores accounts. `sync_user` computes the account identity from the compile-time Django issuer and the membership number:

```rust
let issuer_url = format!("{}{}", DJANGO_OAUTH_BASE_URL, "/o");
let identity = Identity::from_claims(&issuer_url, &mitgliedsnr.to_string());
```

Django's `sub` is `user.pk`. `mitgliedsnr` is that primary key, so a Django ID token matches `account.identity` and `admin_identities` when its `iss` is exactly `{DJANGO_BASE_URL}/o`. A token from another issuer can open a connection, but `ctx.sender` does not match the synced account.

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

