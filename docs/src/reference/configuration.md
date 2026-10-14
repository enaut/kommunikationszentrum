# Configuration Profiles

Kommunikationszentrum uses a compile-time, profile-driven configuration system for the **admin web interface** (`admin`) and the **SpacetimeDB server module** (`server`).

Instead of reading operational environment variables at runtime, configuration parameters (endpoints, OAuth settings, branding, colors, and localized copy) are defined in profile directories located under `profiles/<profile_name>/` and compiled directly into the respective WebAssembly binaries.

---

## Profile Workflow

### 1. Profile Location & Tracking

- **Template Profile**: `profiles/example/` is tracked in git. It provides sensible development defaults and documents all supported fields in `profile.toml` and sample localization overrides in `locales/`.
- **Custom Profiles**: Custom profile directories (such as `profiles/my-org/`, `profiles/solawis/`, or `profiles/uhle/`) are ignored by git via `.gitignore` (`/profiles/*` with `!/profiles/example/`). This prevents accidental leakage of organization-specific endpoints, client IDs, or branding assets into version control.

To create a new profile:

```bash
cp -r profiles/example profiles/my-org
```

### 2. Selecting a Profile at Build Time

Both `admin/build.rs` and `server/build.rs` resolve the profile configuration using the following environment variables:

| Variable | Description | Default |
|---|---|---|
| `APP_PROFILE` | Name of the profile directory in `profiles/` (e.g. `my-org`) | `"example"` |
| `APP_CONFIG_PATH` | Explicit path to a profile directory or profile TOML file (relative to manifest or absolute) | _(none)_ |

If neither variable is specified, the build system defaults to `profiles/example`.

#### Building the Admin Interface with a Profile

```bash
# Using APP_PROFILE
APP_PROFILE=my-org dx build -p admin

# For local development with live reload
APP_PROFILE=my-org dx serve -p admin
```

#### Building and Publishing the Server Module with a Profile

```bash
# Publish module to SpacetimeDB
APP_PROFILE=my-org spacetime publish -p server kommunikation

# Or run cargo tests with a profile
APP_PROFILE=my-org cargo test -p spacetime-module
```

---

## Profile Schema Reference

### `[spacetimedb]`

Specifies SpacetimeDB connection settings for the admin web application.

| Field | Type | Default | Used by | Description |
|---|---|---|---|---|
| `uri` | String | `"http://localhost:3000"` | `admin` | HTTP / WebSocket URL of the SpacetimeDB instance. |
| `module_name` | String | `"kommunikation"` | `admin` | Database / module name published on SpacetimeDB. |

Example:

```toml
[spacetimedb]
uri = "https://spacetime.example.org"
module_name = "kommunikation"
```

---

### `[server]`

Defines backend-specific endpoints for the SpacetimeDB server module.

| Field | Type | Default | Used by | Description |
|---|---|---|---|---|
| `frontend_base_url` | String | Fallback to `[oauth].redirect_uri` (without `/callback`), or `"http://127.0.0.1:8080"` | `server` | Base URL of the admin web frontend. Used to construct self-service email verification links and unsubscribe links. |

Example:

```toml
[server]
frontend_base_url = "https://admin.example.org"
```

---

### `[oauth]`

Configures OpenID Connect (OIDC) authentication. Sourced by both `admin` and `server` to guarantee identity and issuer alignment.

| Field | Type | Default | Used by | Description |
|---|---|---|---|---|
| `issuer_url` | String | `"http://127.0.0.1:8000/o"` | `admin`, `server` | OIDC discovery base URL (where `/.well-known/openid-configuration` resides). In the server module, `register_self` enforces that the JWT `iss` claim exactly matches this URL, and user-sync derives account identities from it. |
| `client_id` | String | `"admin-app"` | `admin` | OAuth client ID registered with the OIDC provider. |
| `redirect_uri` | String | `"http://127.0.0.1:8080/callback"` | `admin`, `server` (fallback) | Callback URL for the OAuth authorization code flow. |
| `scope` | String | `"openid profile email"` | `admin` | Space-separated list of OAuth scopes requested during authorization. |

Example:

```toml
[oauth]
issuer_url = "https://auth.example.org/o"
client_id = "kommunikationszentrum-prod"
redirect_uri = "https://admin.example.org/callback"
scope = "openid profile email"
```

---

### `[brand]`

Customizes brand identity, browser metadata, icons, and visual branding across the admin interface and server module.

| Field | Type | Default | Used by | Description |
|---|---|---|---|---|
| `name` | String | `"Kommunikationszentrum"` | `admin`, `server` | Service / brand name used in navbar, system emails (MTA bounce notifications and verification mail), and default page title. |
| `page_title` | String | `"<name> Admin"` | `admin` | Document title rendered in the browser tab. |
| `icon` | String | `"envelope-fill"` | `admin` | Bootstrap icon name for the navigation bar and login card (e.g. `"envelope-fill"`, `"mortarboard-fill"`). |
| `favicon` | String | `"/assets/favicon.ico"` | `admin` | Path to the favicon relative to admin web assets. |

Example:

```toml
[brand]
name = "UhleCloud Schulmail"
page_title = "UhleCloud Schulmail Admin"
icon = "mortarboard-fill"
favicon = "/assets/favicon.ico"
```

---

### `[brand.theme]`

Defines Bootstrap theme colors and surface tokens for light and dark modes.

#### Light Mode Colors (`[brand.theme.colors]`)

| Token | Default | Description |
|---|---|---|
| `primary` | `"#0d6efd"` | Primary brand accent color. |
| `secondary` | `"#6c757d"` | Secondary interface color. |
| `success` | `"#198754"` | Success status color. |
| `info` | `"#0dcaf0"` | Informational highlight color. |
| `warning` | `"#ffc107"` | Warning status color. |
| `danger` | `"#dc3545"` | Error / destructive action color. |
| `light` | `"#f8f9fa"` | Light background tone. |
| `dark` | `"#212529"` | Dark text / contrast tone. |

#### Light Mode Surfaces (`[brand.theme.surfaces]`)

| Token | Default | Description |
|---|---|---|
| `body_bg` | `"#ffffff"` | Main page background color. |
| `body_color` | `"#212529"` | Primary text color. |
| `secondary_bg` | `"#f8f9fa"` | Sub-card / container background. |
| `secondary_color` | `"#495057"` | Muted text color. |
| `border_color` | `"#dee2e6"` | Divider and card border color. |

#### Dark Mode (`[brand.theme.dark.colors]` and `[brand.theme.dark.surfaces]`)

Configures dark mode overrides. In addition to the surface tokens above, supports:

| Token | Default | Description |
|---|---|---|
| `link_color` | `"#6ea8fe"` | Hyperlink text color in dark mode. |
| `link_hover_color` | `"#9ec5fe"` | Hyperlink hover color in dark mode. |

Example:

```toml
[brand.theme.colors]
primary = "#004b93"

[brand.theme.surfaces]
body_bg = "#f4f7fb"
border_color = "#d0dbe5"
```

---

## Profile Locales (`profiles/<name>/locales/`)

Customizes localized copy directly via native Fluent (`.ftl`) files. Each profile directory must contain:
- `locales/de.ftl`: German locale overrides
- `locales/en.ftl`: English locale overrides

### Overriding Translations

Any translation key defined in `locales/de.ftl` or `locales/en.ftl` overrides the default translation in `admin/src/locales/` natively using Fluent's `add_resource_overriding`. Keys omitted from profile FTL files seamlessly fall back to the base application translations.

### Common Brand Copy Keys

| Key | Used in | Description |
|---|---|---|
| `navbar-brand` | Top navigation bar | Brand name displayed in the navbar header. |
| `app-login-title` | Login screen | Main title on the login card. |
| `app-login-subtitle` | Login screen | Subtitle displayed below the login title. |
| `app-login-button` | Login screen | Text for the OIDC login action button. |
| `community-name` | Topic descriptions | Name of the collective or organization. |
| `subscriptions-required` | Subscriptions page | Explanatory note displayed next to mandatory topic subscriptions. |
| `email-verification-continue` | Email verification | Button text directing the user back to the application after email verification. |
| `unsubscribe-continue` | Unsubscribe page | Button text directing the user back to the application after unsubscribing. |

Example `profiles/my-org/locales/de.ftl`:

```ftl
navbar-brand = UhleCloud Schulmail
app-login-title = UhleCloud Schulmail
app-login-subtitle = Schul-Nachrichtenverwaltung
app-login-button = Mit Schulkonto anmelden
community-name = Schulgemeinschaft
subscriptions-required = Dieses Thema ist für die Teilnahme an der Schulgemeinschaft erforderlich
email-verification-continue = Weiter zur Schulmail
unsubscribe-continue = Weiter zur Schulmail
```

Example `profiles/my-org/locales/en.ftl`:

```ftl
navbar-brand = UhleCloud School Mail
app-login-title = UhleCloud School Mail
app-login-subtitle = School Message Management
app-login-button = Sign in with school account
community-name = School Community
subscriptions-required = This topic is required to participate in the school community
email-verification-continue = Continue to School Mail
unsubscribe-continue = Continue to School Mail
```
