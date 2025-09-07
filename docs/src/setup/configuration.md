# Configuration

The Kommunikationszentrum follows the [12-factor app](https://12factor.net/) methodology, using environment variables for configuration. This approach provides flexibility for different deployment environments while maintaining security best practices.

## Configuration Overview

All components support environment-based configuration with sensible defaults for development. Configuration is loaded in the following order:

1. Environment variables
2. `.env` file (if present)
3. Default values

## Environment Variables

### Core Database Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SPACETIMEDB_URI` | `http://localhost:3000` | SpacetimeDB server endpoint |
| `SPACETIMEDB_MODULE_NAME` | `kommunikation` | SpacetimeDB module name |

### Webhook Proxy Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `WEBHOOK_PROXY_BIND_ADDRESS` | `0.0.0.0:3002` | HTTP server bind address and port |

### OAuth/Authentication Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `DJANGO_BASE_URL` | `http://127.0.0.1:8000` | Django OAuth provider base URL |
| `OIDC_ISSUER_URL` | `http://127.0.0.1:8000/o` | OAuth issuer discovery URL |
| `OIDC_CLIENT_ID` | `admin-app` | OAuth client identifier |
| `ADMIN_REDIRECT_URI` | `http://127.0.0.1:8080/callback` | OAuth callback URL for admin UI |
| `OAUTH_SCOPES` | `openid profile email` | Requested OAuth scopes |

### Logging Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Rust logging level (error, warn, info, debug, trace) |

## Configuration Files

### Development Configuration

For local development, copy the example configuration:

```bash
cp .env.example .env
```

The `.env.example` file contains all variables with development-appropriate defaults:

```ini
# SpacetimeDB Configuration
SPACETIMEDB_URI=http://localhost:3000
SPACETIMEDB_MODULE_NAME=kommunikation

# Webhook Proxy Configuration
WEBHOOK_PROXY_BIND_ADDRESS=0.0.0.0:3002

# Django/OAuth Configuration
DJANGO_BASE_URL=http://127.0.0.1:8000
OIDC_ISSUER_URL=http://127.0.0.1:8000/o
OIDC_CLIENT_ID=admin-app

# Admin Web App Configuration
ADMIN_REDIRECT_URI=http://127.0.0.1:8080/callback
OAUTH_SCOPES=openid profile email

# Logging
RUST_LOG=info
```

### Production Configuration

For production deployments, see `.env.production.example` for guidance:

```ini
# Production values - adjust for your environment
SPACETIMEDB_URI=https://spacetimedb.your-domain.com
DJANGO_BASE_URL=https://auth.your-domain.com
OIDC_CLIENT_ID=kommunikationszentrum-prod
ADMIN_REDIRECT_URI=https://admin.your-domain.com/callback
RUST_LOG=warn
```

## Component-Specific Configuration

### Webhook Proxy

The webhook proxy loads configuration via the `WebhookProxyConfig::load()` method:

```rust
// Automatically loads from .env file and environment variables
let config = WebhookProxyConfig::load()?;
```

Configuration is logged at startup for verification.

### Admin Web Interface

The admin interface loads configuration in the browser:

```rust
// Loads configuration from environment variables set at build time
let config = AdminConfig::load();
```

For WASM targets, environment variables must be set during the build process.

### SpacetimeDB Server Module

Since SpacetimeDB modules run in a sandboxed WASM environment, configuration is set at compile time:

```rust
// Uses compile-time environment variables
const DJANGO_OAUTH_BASE_URL: &str = match option_env!("DJANGO_BASE_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:8000",
};
```

## Security Considerations

### Sensitive Information

- Never commit `.env` files containing sensitive information to version control
- Use different client IDs and secrets for each environment
- In production, prefer environment variables over `.env` files
- Regularly rotate OAuth client secrets

### HTTPS in Production

For production deployments:
- Use HTTPS URLs for all external endpoints
- Configure proper SSL/TLS certificates
- Update OAuth redirect URIs to use HTTPS

## Environment-Specific Setup

### Development

Use the provided defaults for local development:

```bash
# Uses localhost URLs and default ports
cp .env.example .env
```

### Staging

Create environment-specific configuration:

```bash
# staging.env
SPACETIMEDB_URI=https://staging-spacetime.company.com
DJANGO_BASE_URL=https://staging-auth.company.com
OIDC_CLIENT_ID=kommunikationszentrum-staging
```

### Production

Use secure production values:

```bash
# Set via deployment system or CI/CD
export SPACETIMEDB_URI=https://spacetime.company.com
export DJANGO_BASE_URL=https://auth.company.com
export OIDC_CLIENT_ID=kommunikationszentrum-prod
export RUST_LOG=warn
```

## Troubleshooting Configuration

### Verification

Check configuration loading by examining startup logs:

```bash
# Webhook proxy logs configuration at startup
cargo run --package webhook-proxy
```

### Common Issues

**Connection failures**: Verify `SPACETIMEDB_URI` is correct and SpacetimeDB is running.

**OAuth errors**: Check `DJANGO_BASE_URL` and `OIDC_CLIENT_ID` match your OAuth provider configuration.

**Port conflicts**: Ensure `WEBHOOK_PROXY_BIND_ADDRESS` uses an available port.

### Configuration Validation

Test configuration without starting full services:

```bash
# Test webhook proxy configuration
SPACETIMEDB_URI=http://test:3000 cargo check --package webhook-proxy

# Test admin configuration  
DJANGO_BASE_URL=http://test:8000 cargo check --package admin
```