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
926  curl --proto '=https' --tlsv1.2 -sSf https://get.stalw.art/install.sh -o install.sh
927  sudo sh install.sh  # install stalwart
928  journalctl -u stalwart # get initial password
929  systemctl restart stalwart # restart stalwart after initial setup
```

* change listener port to 8093
* create users
* setup mta-webhook

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
