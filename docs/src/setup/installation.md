# Setup & Installation

```bash
sudo dnf install b3sum
curl -f https://zed.dev/install.sh | sh
curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
curl -fsSL https://d2lang.com/install.sh | sh -s -- --tala
curl -sSf https://install.spacetimedb.com | sh
cargo install-update --all
cargo binstall trunk
cargo install-update --all
cargo binstall dioxus-cli
cargo binstall mdbook mdbook-d2
git clone git@github.com:enaut/kommunikationszentrum.git

```

```bash
curl --proto '=https' --tlsv1.2 -sSf https://get.stalw.art/install.sh -o install.sh
sudo sh install.sh         # install Stalwart MTA
journalctl -u stalwart     # retrieve initial admin credentials
systemctl restart stalwart # restart stalwart after initial setup
```

Follow the complete [Stalwart MTA Setup](../email/stalwart-setup.md) guide for configuration with screenshots:
* **Listeners**: [Change HTTP listener port to 8093](../email/stalwart-setup.md#listeners-configuration-port-8093) (avoids port collisions; targets Prometheus scraping).
* **Admin API Key**: [Create `webportal admin` API key](../email/stalwart-setup.md#admin-api-key-configuration-stalwart_admin_token) (provides `STALWART_ADMIN_TOKEN` for SpacetimeDB JMAP domain and category sync).
* **MTA Webhook**: [Configure MTA Hook in Stalwart UI](../email/stalwart-setup.md#mta-hook-configuration) (using a bearer token generated via [Managing Webhook Tokens](../core/spacetimedb/module-publishing.md#managing-webhook-tokens)).
* **Telemetry**: [Enable Prometheus metrics & OpenTelemetry tracing](../email/stalwart-setup.md#monitoring--telemetry).

# tasks

ctrl+shift+p→open tasks

```json
[
  {
    "label": "Start SoLaWiS",
    "command": "/path/to/python /path/to/solawispielplatz/src/manage.py runserver", # get env pythonpath with `which python`
    "cwd": "$ZED_WORKTREE_ROOT",
    "use_new_terminal": true,
    "allow_concurrent_runs": false,
    "reveal": "always",
    "hide": "never",
    "show_summary": true,
    "show_command": true,
    "save": "all",
  },
  {
    "label": "Start SpacetimeDB",
    "command": "spacetime start",
    "cwd": "$ZED_WORKTREE_ROOT",
    "use_new_terminal": true,
    "allow_concurrent_runs": false,
    "reveal": "always",
    "hide": "never",
    "show_summary": true,
    "show_command": true,
    "save": "all",
  },
]
```

```bash
curl -sSf https://install.spacetimedb.com | sh # install spacetime
spacetime logspacetime server set-default local
spacetime login show --token # show login token → put it in the django settings
openssl genrsa -out oidc_private.pem 4096 # generate oidc private key
cat oidc_private.pem # show oidc private key → put it in the django settings
```
