# Environment Variables Reference

Complete reference of all environment variables supported by Kommunikationszentrum components.

## Core Configuration

### SpacetimeDB Connection

#### `SPACETIMEDB_URI`
- **Default**: `http://localhost:3000`
- **Used by**: webhook-proxy, admin
- **Description**: URI for the SpacetimeDB server
- **Format**: `http://host:port` or `https://host:port`
- **Examples**:
  - Development: `http://localhost:3000`
  - Production: `https://spacetime.company.com`

#### `SPACETIMEDB_MODULE_NAME`
- **Default**: `kommunikation`
- **Used by**: webhook-proxy, admin
- **Description**: Name of the SpacetimeDB module
- **Format**: String identifier
- **Examples**: `kommunikation`, `email-prod`, `test-module`

## Network Configuration

### Webhook Proxy Server

#### `WEBHOOK_PROXY_BIND_ADDRESS`
- **Default**: `0.0.0.0:3002`
- **Used by**: webhook-proxy
- **Description**: Address and port for the HTTP server
- **Format**: `host:port`
- **Examples**:
  - Listen on all interfaces: `0.0.0.0:3002`
  - Listen on localhost only: `127.0.0.1:3002`
  - Custom port: `0.0.0.0:8080`

## Authentication Configuration

### OAuth Provider

#### `DJANGO_BASE_URL`
- **Default**: `http://127.0.0.1:8000`
- **Used by**: admin, server (compile-time)
- **Description**: Base URL of the Django OAuth provider
- **Format**: `http://host:port` or `https://host:port`
- **Examples**:
  - Development: `http://127.0.0.1:8000`
  - Production: `https://auth.company.com`

#### `OIDC_ISSUER_URL`
- **Default**: `http://127.0.0.1:8000/o`
- **Used by**: admin
- **Description**: OAuth issuer discovery endpoint
- **Format**: `http://host:port/path` or `https://host:port/path`
- **Notes**: Usually `DJANGO_BASE_URL` + `/o` for Django OAuth Toolkit

#### `OIDC_CLIENT_ID`
- **Default**: `admin-app`
- **Used by**: admin
- **Description**: OAuth client identifier
- **Format**: String identifier
- **Security**: Should be unique per environment
- **Examples**: `admin-app`, `kommunikationszentrum-prod`

### OAuth Flow Configuration

#### `ADMIN_REDIRECT_URI`
- **Default**: `http://127.0.0.1:8080/callback`
- **Used by**: admin
- **Description**: OAuth callback URL
- **Format**: Complete URL with protocol
- **Requirements**: Must be registered with OAuth provider
- **Examples**:
  - Development: `http://127.0.0.1:8080/callback`
  - Production: `https://admin.company.com/callback`

#### `OAUTH_SCOPES`
- **Default**: `openid profile email`
- **Used by**: admin
- **Description**: Space-separated OAuth scopes
- **Format**: `scope1 scope2 scope3`
- **Required**: `openid` must be included
- **Common scopes**: `profile`, `email`, `groups`

## Logging Configuration

#### `RUST_LOG`
- **Default**: `info`
- **Used by**: All Rust components
- **Description**: Rust tracing/logging level
- **Values**: `error`, `warn`, `info`, `debug`, `trace`
- **Module-specific**: `webhook_proxy=debug,admin=info`

## Development-Only Variables

### Testing and Debugging

#### `DEBUG_MODE`
- **Default**: Not set
- **Used by**: Development builds
- **Description**: Enables additional debug features
- **Values**: `true`, `1`, `on`

## Deployment Examples

### Local Development
```bash
SPACETIMEDB_URI=http://localhost:3000
DJANGO_BASE_URL=http://127.0.0.1:8000
RUST_LOG=debug
```

### Docker Compose
```bash
SPACETIMEDB_URI=http://spacetime:3000
DJANGO_BASE_URL=http://django:8000
WEBHOOK_PROXY_BIND_ADDRESS=0.0.0.0:3002
```

### Kubernetes Production
```bash
SPACETIMEDB_URI=https://spacetime.company.com
DJANGO_BASE_URL=https://auth.company.com
OIDC_CLIENT_ID=kommunikationszentrum-k8s
ADMIN_REDIRECT_URI=https://admin.company.com/callback
RUST_LOG=warn
```

## Configuration Validation

### Required Variables
These variables must be set for production deployments:
- `SPACETIMEDB_URI`
- `DJANGO_BASE_URL`
- `OIDC_CLIENT_ID`
- `ADMIN_REDIRECT_URI`

### Security Checklist
- [ ] `OIDC_CLIENT_ID` is environment-specific
- [ ] All URLs use HTTPS in production
- [ ] `RUST_LOG` is set to `warn` or `error` in production
- [ ] OAuth redirect URIs are registered with the provider

### Common Validation Commands
```bash
# Check webhook proxy configuration
cargo run --package webhook-proxy -- --help

# Validate admin configuration
cargo check --package admin

# Test SpacetimeDB connection
spacetime call kommunikation get_mta_logs
```