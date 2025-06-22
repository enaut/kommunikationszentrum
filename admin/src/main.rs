mod module_bindings;
mod use_spacetime_db;

use dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use module_bindings::*;
use spacetimedb_sdk::DbContext as _;
use use_spacetime_db::{use_accounts_table, use_spacetime_db, ConnectionState, SpacetimeDbOptions};

use crate::use_spacetime_db::use_spacetime_subscription;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const HEADER_SVG: Asset = asset!("/assets/header.svg");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Use the SpacetimeDB hook
    let spacetime_db = use_spacetime_db(SpacetimeDbOptions {
        uri: "http://localhost:3000".to_string(),
        module_name: "kommunikationszentrum".to_string(),
        creds_file: None,
        token: None,
    });

    let _subsc =
        use_spacetime_subscription(&spacetime_db, vec!["SELECT * FROM account".to_string()]);

    // Use the new reactive accounts hook instead of memo
    let accounts = use_accounts_table(&spacetime_db);

    // Provide the spacetime_db as context
    use_context_provider(|| spacetime_db.clone());

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        match &*spacetime_db.state.read() {
            ConnectionState::Connected(identity) => rsx! {
                div { "Connected to database successfully! Identity: {identity:?}" }
                div { "Number of accounts: {accounts.read().len()}" }
                Hero { accounts }
            },
            ConnectionState::Error(e) => rsx! {
                div { style: "color: red; padding: 10px; margin: 10px; border: 1px solid red;",
                    "Failed to connect to database: {e}"
                    p { "Make sure SpacetimeDB server is running on localhost:3000" }
                }
                Hero { accounts }
            },
            ConnectionState::Connecting => rsx! {
                div { "Connecting to database... (check console for details)" }
                Hero { accounts }
            },
            ConnectionState::Disconnected => rsx! {
                div { "Disconnected from database" }
                Hero { accounts }
            },
        }
    }
}

#[component]
pub fn Hero(accounts: Signal<Vec<Account>>) -> Element {
    let spacetime_db: use_spacetime_db::SpacetimeDb = use_context();

    let db_status = match &*spacetime_db.state.read() {
        ConnectionState::Connected(_) => {
            let accounts_count = accounts.read().len();
            info!(
                "Database connection is active with {} accounts",
                accounts_count
            );
            format!("Connected! {} accounts loaded", accounts_count)
        }
        ConnectionState::Error(e) => {
            info!("Database connection failed: {:?}", e);
            format!("Failed to connect to database: {}", e)
        }
        ConnectionState::Connecting => "Connecting...".to_string(),
        ConnectionState::Disconnected => "Disconnected".to_string(),
    };

    rsx! {
        div { id: "hero",
            img { src: HEADER_SVG, id: "header" }
            div { id: "links",
                a { href: "https://dioxuslabs.com/", "🔄 {db_status}" }
                for user in accounts.read().iter() {
                    button {
                        onclick: {
                            let spacetime_db = spacetime_db.clone();
                            let user_name = user.name.clone();
                            let user_email = user.email.clone();
                            move |_| {
                                info!("Adding message category for user: {} ({})", user_name, user_email);
                                if let Some(db) = spacetime_db.connection.as_ref() {
                                    match db
                                        .reducers
                                        .add_message_category(
                                            user_name.clone(),
                                            user_email.clone(),
                                            "No Description".to_string(),
                                        )
                                    {
                                        Ok(_) => {
                                            info!("Successfully added message category for {}", user_name);
                                        }
                                        Err(e) => {
                                            error!(
                                                "Failed to add message category for {}: {:?}", user_name, e
                                            );
                                        }
                                    }
                                }
                            }
                        },
                        style: "cursor: pointer; padding: 8px 12px; margin: 4px; background: #007acc; color: white; border: none; border-radius: 4px; text-decoration: none; display: inline-block;",
                        "👤 {user.name} (Add Category)"
                    }
                }
            }
        }
    }
}
