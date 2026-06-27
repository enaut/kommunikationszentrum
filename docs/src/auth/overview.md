# Authentication & Security

The Kommunikationszentrum implements a comprehensive authentication and authorization system based on OAuth 2.0 with OpenID Connect (OIDC). The system provides secure access to the admin interface and protects SpacetimeDB operations through JWT-based authentication.

## System Overview

The authentication architecture integrates three core components:

- **Django OAuth2 Provider** (solawispielplatz): Issues JWT ID tokens via OpenID Connect
- **SpacetimeDB Server**: Validates JWT tokens and manages authenticated connections
- **Admin Web Interface**: Uses JWT tokens for authenticated database operations

## Key Features

**OAuth 2.0 with PKCE**: Secure authorization flow with Proof Key for Code Exchange to prevent authorization code interception attacks.

**OpenID Connect Integration**: JWT ID tokens contain user identity and permission claims from Django, providing seamless identity propagation.

**Nonce Validation**: Each authorization request includes a cryptographically random nonce that is validated against the ID token to prevent replay and token substitution.

**Real-time Authentication**: SpacetimeDB validates JWT tokens on connection establishment, enabling secure real-time database subscriptions.

**Role-based Access Control**: Different permission levels for regular users and administrators based on Django user roles.

## Authentication Flow

```d2
direction: right
User: "User" { style.fill: lightyellow }
AdminUI: "Admin Web Interface" { style.fill: lightgreen }
Django: "Django OAuth2 Provider" { style.fill: lightpink }
SpacetimeDB: "SpacetimeDB Server" { style.fill: lightcyan }

User -> AdminUI: "Initiate Login" { style.stroke: blue }
AdminUI -> Django: "OAuth2 Authorization Request" { style.stroke: blue }
Django -> User: "User Authenticates" { style.stroke: blue }
Django -> AdminUI: "Return Authorization Code" { style.stroke: blue }
AdminUI -> Django: "Token Exchange" { style.stroke: blue }
Django -> AdminUI: "Access Token & JWT ID Token" { style.stroke: blue }
AdminUI -> SpacetimeDB: "Send JWT ID Token" { style.stroke: blue }
SpacetimeDB -> Django: "Validate JWT" { style.stroke: blue }
SpacetimeDB -> AdminUI: "Authenticated Connection" { style.stroke: blue }
```
