# Admin Interface Setup

The **Admin Interface** (`admin`) is a Single-Page Application (SPA) built with [Dioxus](https://dioxuslabs.com/) and compiled to WebAssembly (Wasm). It provides topic subscription management, recipient administrative controls, and system monitoring in the browser.

---

## Prerequisites

- **Rust toolchain** (stable) with `wasm32-unknown-unknown` target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- **Dioxus CLI** (`dx`):
  ```bash
  cargo install dioxus-cli
  ```

---

## Configuration & Profiles

The admin interface uses compile-time profile configuration. Endpoints, OAuth client settings, visual branding (colors, surface tones, logo icon, page title), and localized copy are resolved at build time from a TOML profile file located in `profiles/`.

1. Copy the template profile if you have not already created one:
   ```bash
   cp profiles/example.toml profiles/my-org.toml
   ```
2. Customize the `[spacetimedb]`, `[oauth]`, `[brand]`, `[brand.theme]`, and `[brand.i18n]` sections in `profiles/my-org.toml`.
3. Build or serve the application with `APP_PROFILE`:

> [!TIP]
> For the complete schema, supported fields, theme tokens, and Fluent localization keys, see the [Configuration Profiles Reference](../reference/configuration.md).

---

## Development Server

To run the admin interface locally with live hot-reloading:

```bash
# Serves the app at http://127.0.0.1:8080 (or port configured in Dioxus.toml)
APP_PROFILE=my-org dx serve -p admin
```

If `APP_PROFILE` is omitted, the build defaults to `profiles/example.toml`.

---

## Building for Production

To create an optimized WebAssembly bundle for deployment:

```bash
APP_PROFILE=my-org dx build -p admin --release
```

The resulting assets and Wasm bundle are output to `admin/dist/` (or target directory defined in `admin/Dioxus.toml`), ready to be served by any static file server or reverse proxy (e.g. Nginx, Caddy, or Stalwart).
