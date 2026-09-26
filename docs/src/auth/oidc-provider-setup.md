# OpenID Connect (OIDC) Provider Setup

Kommunikationszentrum is designed to work with **any standard-compliant OpenID Connect (OIDC) provider**, such as Nextcloud, Keycloak, Authentik, or Django with `django-oauth-toolkit`.

## How Authentication Works

1. **Client Authorization**: The frontend single-page app (Dioxus WebAssembly) initiates the OAuth 2.0 Authorization Code Flow with PKCE against the OIDC provider's authorization endpoint.
2. **Token Exchange**: Upon callback, the frontend exchanges the authorization code for an ID token (JWT) and an access token, and fetches claims from the userinfo endpoint.
3. **Database Connection**: The frontend passes the OIDC JWT ID token to SpacetimeDB as its authentication bearer token.
4. **Identity Derivation**: SpacetimeDB validates the token against the OIDC provider's JWKS and derives a deterministic 32-byte cryptographic `Identity` based on `(iss, sub)`.
5. **Self-Registration**: Upon connecting, the web app calls the `register_self` reducer to create or link the user's `Account` in SpacetimeDB with `external_id` (the OIDC `sub` claim), full name, and email.

---

## Configuring SpacetimeDB & Admin

Set the following environment variables when building/deploying:

| Variable | Description | Example |
|---|---|---|
| `OIDC_ISSUER_URL` | Issuer URL of the OIDC provider (where `/.well-known/openid-configuration` resides) | `https://cloud.example.org` |
| `OIDC_CLIENT_ID` | Client ID registered in the OIDC provider | `admin-app` |
| `ADMIN_REDIRECT_URI` | Callback URL for OAuth redirection | `https://kom.example.org/callback` |
| `OAUTH_SCOPES` | Scopes requested during authorization | `openid profile email` |

---

## Provider Configurations

### 1. Nextcloud (OIDC / OAuth2 App)

1. In Nextcloud, install the **OAuth2 / OIDC** app (or use the built-in OAuth2 client registration).
2. Go to **Administration Settings > Security > OAuth 2.0 clients**.
3. Add a new client:
   - **Name**: `Kommunikationszentrum`
   - **Redirection URI**: `http://localhost:8080/callback` (or your production frontend URL)
4. Configure your environment:
   ```bash
   export OIDC_ISSUER_URL="https://cloud.yourdomain.org"
   export OIDC_CLIENT_ID="<client-id-from-nextcloud>"
   export ADMIN_REDIRECT_URI="https://kom.yourdomain.org/callback"
   export OAUTH_SCOPES="openid profile email"
   ```

### 2. Keycloak

1. Create a realm (e.g. `community`) or use an existing one.
2. Go to **Clients > Create client**:
   - **Client type**: OpenID Connect
   - **Client ID**: `admin-app`
   - **Client authentication**: Off (Public client)
   - **Authentication flow**: Standard flow (Authorization Code) + Direct access grants
   - **Valid redirect URIs**: `http://localhost:8080/callback`, `https://kom.yourdomain.org/callback`
   - **Web origins**: `+` (or your frontend domain)
3. Configure your environment:
   ```bash
   export OIDC_ISSUER_URL="https://keycloak.yourdomain.org/realms/community"
   export OIDC_CLIENT_ID="admin-app"
   export ADMIN_REDIRECT_URI="https://kom.yourdomain.org/callback"
   export OAUTH_SCOPES="openid profile email"
   ```

### 3. Django (`django-oauth-toolkit` / Solawispielplatz)

1. Register an application in Django admin under `Django OAuth Toolkit > Applications`:
   - **Client type**: Public
   - **Authorization grant type**: Authorization code
   - **Redirect URIs**: `http://127.0.0.1:8080/callback`
   - **Algorithm**: RSA with SHA-2 256
2. Configure your environment:
   ```bash
   export OIDC_ISSUER_URL="http://127.0.0.1:8000/o"
   export OIDC_CLIENT_ID="admin-app"
   export ADMIN_REDIRECT_URI="http://127.0.0.1:8080/callback"
   export OAUTH_SCOPES="openid profile email"
   ```

---

## Registering Administrators

When deploying a fresh instance or adding new administrators:

1. When you publish the module with `spacetime publish`, your SpacetimeDB CLI identity is automatically seeded as an administrator in `admin_identities` by the `init` lifecycle reducer.
2. Log into the web application via OIDC. Your account is automatically provisioned via `register_self`.
3. Inspect your assigned SpacetimeDB `Identity` in the application status page or via CLI logs.
4. As the operator, use your CLI identity to promote the web application identity:
   ```bash
   spacetime call kommunikation register_admin_identity "<your_identity_hex>"
   ```
5. Once registered, you have administrative access in the Admin UI and can promote other users directly within the interface.

> [!NOTE]
> For deploying this release against an existing database, perform a schema reset via:
> ```bash
> spacetime publish -c -p server kommunikation
> ```
