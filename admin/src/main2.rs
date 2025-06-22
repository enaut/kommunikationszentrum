mod module_bindings;
mod use_spacetime_db;

use dioxus::{logger::tracing::info, prelude::*};
use module_bindings::*;
use spacetimedb_sdk::{DbContext as _, Table as _};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const HEADER_SVG: Asset = asset!("/assets/header.svg");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut num_of_accounts: Signal<u64> = use_signal(|| 0);

    // Spawn a local effect to update the signal when the channel receives a value
    let receiver = use_coroutine(move |mut rx: UnboundedReceiver<u64>| async move {
        use futures_util::StreamExt;

        while let Some(count) = rx.next().await {
            info!("Received account count update: {}", count);
            *num_of_accounts.write() = count;
        }
    });
    let tx = receiver.tx();

    let db_connection = use_resource(move || {
        let tx = tx.clone();
        async move {
            info!("Initializing database connection...");
            let database = DbConnection::builder()
                .with_module_name("kommunikationszentrum")
                .with_uri("http://localhost:3000")
                .with_light_mode(true)
                .on_connect(move |ctx, id, token| {
                    info!("Connected to database with ID: {id} and token: {token}");
                    ctx.subscription_builder()
                        .subscribe(["SELECT * FROM account where id < 100"]);
                });
            info!("Connecting to database at http://localhost:3000");

            match database.build().await {
                Ok(db) => {
                    info!("Database connection established successfully.");
                    db.run_background();

                    // Move tx into the closure to send updates
                    let tx2 = tx.clone();
                    db.db().account().on_insert(move |ctx, _table| {
                        info!("Inserted row into table {:?}", ctx.event);
                        let count = ctx.db().account().count();
                        info!("Total accounts after insert: {}", count);
                        let send_result = tx2.unbounded_send(count);
                        info!(
                            "Sent account count update: {}. Result: {:?}",
                            count, send_result
                        );
                    });

                    tx.unbounded_send(20).ok();

                    let acs: Vec<_> = db.db().account().iter().collect();
                    info!("Retrieved accounts: {:?}", acs);
                    info!("Database connection and subscription initialized.");

                    Ok(db)
                }
                Err(e) => {
                    info!("Failed to connect to database: {:?}", e);
                    Err(e)
                }
            }
        }
    });

    let accounts = use_memo(move || {
        let num_of_accounts = num_of_accounts.read();
        info!("Fetching accounts, current count: {}", num_of_accounts);
        db_connection.read().as_ref().map(|db| match db {
            Ok(db) => db.db().account().iter().collect::<Vec<_>>(),
            Err(_) => Vec::new(),
        })
    });
    use_context_provider(|| db_connection.clone());

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        match &*db_connection.read() {
            Some(Ok(_db)) => rsx! {
                div { "Connected to database successfully!" }
                div { "Number of accounts: {num_of_accounts}" }
                div { "Number of accounts: {accounts:?}" }
                Hero { accounts }
            },
            Some(Err(e)) => rsx! {
                div { style: "color: red; padding: 10px; margin: 10px; border: 1px solid red;",
                    "Failed to connect to database: {e:?}"
                    p { "Make sure SpacetimeDB server is running on localhost:3000" }
                }
                Hero { accounts }
            },
            None => rsx! {
                div { "Connecting to database..." }
                Hero { accounts }
            },
        }
    }
}

#[component]
pub fn Hero(accounts: Memo<Option<Vec<Account>>>) -> Element {
    let db_connection: Resource<Result<DbConnection, spacetimedb_sdk::Error>> = use_context();

    let db_status = match &*db_connection.read() {
        Some(Ok(_db)) => {
            info!("Database connection is active: {:?}", accounts.read());
            info!("Database connection is active.");
            "Connected to database successfully!".to_string()
        }
        Some(Err(e)) => {
            info!("Database connection failed: {:?}", e);
            format!("Failed to connect to database: {:?}", e)
        }
        None => "Connecting...".to_string(),
    };

    rsx! {
        div { id: "hero",
            img { src: HEADER_SVG, id: "header" }
            div { id: "links",
                a { href: "https://dioxuslabs.com/", "🔄 {db_status}" }
                if let Some(Ok(db)) = &*db_connection.read() {
                    for user in db.db().account().iter() {
                        a { href: format!("https://dioxuslabs.com/user/{}", user.id),
                            "👤 {user.name}"
                        }
                    }
                } else {
                    a { href: "https://dioxuslabs.com/learn/0.6/", "📚 Learn Dioxus" }
                    a { href: "https://dioxuslabs.com/awesome", "🚀 Awesome Dioxus" }
                    a { href: "https://github.com/dioxus-community/", "📡 Community Libraries" }
                    a { href: "https://github.com/DioxusLabs/sdk", "⚙️ Dioxus Development Kit" }
                    a { href: "https://marketplace.visualstudio.com/items?itemName=DioxusLabs.dioxus",
                        "💫 VSCode Extension"
                    }
                    a { href: "https://discord.gg/XgGxMSkvUM", "👋 Community Discord" }
                }
            }
        }
    }
}
