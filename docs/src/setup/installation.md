# Setup & Installation

```bash
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