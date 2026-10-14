# Configuration Profiles

Kommunikationszentrum uses a compile-time, profile-driven configuration system for the **admin web interface** (`admin`) and the **SpacetimeDB server module** (`server`).

Instead of reading environment variables at runtime, configuration parameters (endpoints, OAuth settings, branding, colors, and localized copy) are defined in TOML profile files located in the `profiles/` directory and compiled directly into the respective WebAssembly binaries.

---

## Profile Workflow

### 1. Profile Location & Tracking

- **Template Profile**: [`profiles/example.toml`](file:///home/dietrich/Projekte/Source/kommunikationszentrum/profiles/example.toml) is tracked in git. It provides sensible development defaults and documents all supported fields.
- **Custom Profiles**: Custom profile files (such as `profiles/my-org.toml`, `profiles/solawis.toml`, or `profiles/uhle.toml`) are ignored by git via `.gitignore` (`/profiles/*.toml` with `!/profiles/example.toml`). This prevents accidental leakage of organization-specific endpoints, client IDs, or branding assets into version control.

To create a new profile:

```bash
cp profiles/example.toml profiles/my-org.toml
```

### 2. Selecting a Profile at Build Time

Both `admin/build.rs` and `server/build.rs` resolve the profile configuration using the following environment variables:

| Variable | Description | Default |
|---|---|---|
| `APP_PROFILE` | Name of the profile file in `profiles/` (e.g. `my-org` or `my-org.toml`) | `"example"` |
| `APP_CONFIG_PATH` | Explicit path to a profile TOML file (relative to manifest or absolute) | _(none)_ |

If neither variable is specified, the build system defaults to `profiles/example.toml`.

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

Customizes browser metadata, icons, and visual branding in the admin interface.

| Field | Type | Default | Used by | Description |
|---|---|---|---|---|
| `page_title` | String | `"Kommunikationszentrum Admin"` | `admin` | Document title rendered in the browser tab. |
| `icon` | String | `"envelope-fill"` | `admin` | Bootstrap icon name for the navigation bar and login card (e.g. `"envelope-fill"`, `"mortarboard-fill"`). |
| `favicon` | String | `"/assets/favicon.ico"` | `admin` | Path to the favicon relative to admin web assets. |

Example:

```toml
[brand]
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

### `[brand.i18n]`

Customizes localized brand copy. Localized terms are compiled directly into Fluent translation bundles (`-brand-*` terms in `de.ftl` and `en.ftl`) for `admin`, and used in system emails (rejection notices and verification emails) in `server`.

#### Supported Language Sections

- `[brand.i18n.de]`: German brand terms.
- `[brand.i18n.en]`: English brand terms.

#### Fields

| Field | Used by | Description |
|---|---|---|
| `brand_name` | `admin`, `server` | Service name (used in navbar, rejection messages, and verification mail subjects). |
| `login_title` | `admin` | Main title on the login screen. |
| `login_subtitle` | `admin` | Subtitle displayed below the login title. |
| `login_button` | `admin` | Text for the OIDC login action button. |
| `community_name` | `admin` | Name of the community or collective (e.g. `"Gemeinschaft"`, `"Schulgemeinschaft"`). |
| `subscriptions_required` | `admin` | Explanatory note displayed next to mandatory topic subscriptions. |
| `email_verification_continue` | `admin` | Button text directing the user back to the application after email verification. |

Example:

```toml
[brand.i18n.de]
brand_name = "UhleCloud Schulmail"
login_title = "UhleCloud Schulmail"
login_subtitle = "Nachrichtenverwaltung"
login_button = "Mit Schulkonto anmelden"
community_name = "Schulgemeinschaft"
subscriptions_required = "Dieses Thema ist für die Teilnahme an der Schulgemeinschaft erforderlich"
email_verification_continue = "Weiter zur Schulmail"

[brand.i18n.en]
brand_name = "UhleCloud School Mail"
login_title = "UhleCloud School Mail"
login_subtitle = "Message Management"
login_button = "Sign in with school account"
community_name = "School Community"
subscriptions_required = "This topic is required to participate in the school community"
email_verification_continue = "Continue to School Mail"
```
