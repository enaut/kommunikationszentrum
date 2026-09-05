mod config;
mod mail;
mod module_bindings;

use config::SenderConfig;
use lettre::Transport;
use mail::{
    build_transport, compose_delivery, is_permanent_error, is_transient_error,
    resolve_category_smtp_credentials,
};
use module_bindings::{
    claim_next_mail_delivery, claim_next_mail_ingress, complete_mail_ingress,
    enqueue_mail_delivery, ensure_subscription_unsubscribe_token, fail_mail_delivery,
    fail_mail_ingress, mark_mail_delivery_sent, retry_mail_ingress, schedule_mail_delivery_retry,
    DbConnection, MailDeliveryClaimed, MailIngress, MailMessage, MessageCategory, Subscription,
    SubscriptionStatus,
};
use spacetimedb_sdk::{DbContext, Table, TableWithPrimaryKey as _};
use std::{error::Error, sync::Arc};
use uuid::Uuid;

use crate::module_bindings::{
    increment_mail_ingress_delivery_count, ActiveSubscriptionsTableAccess as _,
    ActiveUnsubscribeTokensTableAccess as _, SenderMailDeliveryClaimedTableAccess as _,
    SenderMailDeliveryDoneTableAccess as _, SenderMailDeliveryMessagesTableAccess as _,
    SenderMailDeliveryPendingTableAccess as _, SenderMailIngressTableAccess as _,
    SenderMailMessagesTableAccess as _, VisibleAdminIdentitiesTableAccess as _,
    VisibleMessageCategoriesTableAccess as _,
};
use opentelemetry::global;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig as _;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing::{error, info, instrument, trace, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Whether a subscription with this status should currently receive mail.
/// Mirrors `SubscriptionStatus::is_active` on the server.
fn is_active_subscription(status: &SubscriptionStatus) -> bool {
    matches!(
        status,
        SubscriptionStatus::AutomaticallySubscribed
            | SubscriptionStatus::ManuallySubscribed
            | SubscriptionStatus::RequiredSubscribed
    )
}

/// Events sent through the main channel.
enum Event {
    /// Wake the processing loop to check for new work.
    Wakeup,
    /// A fatal error occurred that cannot be recovered from.
    FatalError(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Arc::new(SenderConfig::from_env());

    init_tracing(&config)?;
    info!("Starting sender with config: {:?}", config);

    let instance_id = Uuid::new_v4().to_string();
    info!("Sender instance_id: {}", instance_id);

    // Channel to wake up the main processing loop when SpacetimeDB data changes.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();

    let connection = connect_to_spacetimedb(&config, tx.clone()).await?;

    info!("Entering main processing loop");

    loop {
        trace!("Main loop running: checking all work...");

        let mut work_done = false;

        // Process ingress jobs
        match process_fanout_jobs(&connection, &config, &instance_id) {
            Ok(did_work) => {
                trace!("process_fanout_jobs returned: did_work={did_work}");
                work_done |= did_work;
            }
            Err(e) => {
                warn!("Error during process_fanout_jobs: {e}");
            }
        }

        // Process claimed delivery jobs (send emails)
        match send_delivery_jobs(&connection, &config, &instance_id) {
            Ok(()) => {}
            Err(e) => {
                warn!("Error during send_delivery_jobs: {e}");
            }
        }

        // Claim next pending delivery job
        trace!("Requesting next mail delivery claim");
        if let Err(error) = connection
            .reducers()
            .claim_next_mail_delivery(instance_id.clone())
        {
            warn!("claim_next_mail_delivery failed: {:?}", error);
        }

        // If any work was completed in this cycle, loop immediately
        // without waiting for an event, so we drain the queue quickly.
        if work_done {
            trace!("Work was done this cycle, immediately checking for more...");
            tokio::task::yield_now().await;
            continue;
        }

        // No work was done: wait for a change event or the periodic fallback timeout.
        trace!("No immediate work done, awaiting next event or tick...");
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(Event::Wakeup) => {
                        trace!("Wakeup event received from SpacetimeDB table update");
                    }
                    Some(Event::FatalError(msg)) => {
                        error!("Fatal error received: {msg}");
                        return Err(msg.into());
                    }
                    None => {
                        info!("Event channel closed, shutting down sender");
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(15)) => {
                trace!("Periodic poll timer ticked (every 15s)");
            }
        }
    }

    Ok(())
}

fn init_tracing(config: &SenderConfig) -> Result<(), Box<dyn Error>> {
    let tracer_provider = if !config.otlp_endpoint.is_empty() {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(config.otlp_endpoint.clone())
            .build()?;

        let resource = Resource::builder_empty()
            .with_service_name("sender".to_string())
            .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
            .build();

        let tracer_provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource)
            .build();

        global::set_tracer_provider(tracer_provider.clone());
        global::set_text_map_propagator(TraceContextPropagator::new());
        Some(tracer_provider)
    } else {
        None
    };

    let log_provider = if !config.otlp_endpoint.is_empty() {
        let exporter = opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .with_endpoint(config.otlp_endpoint.clone())
            .build()?;

        let resource = Resource::builder_empty()
            .with_service_name("sender".to_string())
            .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
            .build();

        let log_provider = SdkLoggerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource)
            .build();

        Some(log_provider)
    } else {
        None
    };

    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer());

    let subscriber = if let Some(provider) = tracer_provider {
        use opentelemetry::trace::TracerProvider as _;
        let tracer = provider.tracer("sender");
        let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
        Some(subscriber.with(telemetry))
    } else {
        None
    };

    let subscriber = if let Some(provider) = log_provider {
        let opentelemetry_appender =
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&provider);
        if let Some(sub) = subscriber {
            sub.with(opentelemetry_appender).try_init()?
        } else {
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::EnvFilter::builder()
                        .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                        .from_env_lossy(),
                )
                .with(tracing_subscriber::fmt::layer())
                .with(opentelemetry_appender)
                .try_init()?
        }
    } else if let Some(sub) = subscriber {
        sub.try_init()?
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::builder()
                    .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                    .from_env_lossy(),
            )
            .with(tracing_subscriber::fmt::layer())
            .try_init()?
    };

    let _ = subscriber;
    Ok(())
}

async fn connect_to_spacetimedb(
    config: &SenderConfig,
    tx: tokio::sync::mpsc::UnboundedSender<Event>,
) -> Result<DbConnection, Box<dyn Error>> {
    let auth_token = config.spacetimedb_token.clone();
    let mut connection_builder = DbConnection::builder()
        .with_uri(&config.spacetimedb_uri)
        .with_database_name(&config.spacetimedb_database_name);

    if let Some(token) = auth_token {
        connection_builder = connection_builder.with_token(Some(token));
    }

    let connection_token = Arc::new(std::sync::Mutex::new(None));
    let token_clone = connection_token.clone();

    let (connected_tx, connected_rx) = tokio::sync::oneshot::channel();
    let connected_tx = Arc::new(std::sync::Mutex::new(Some(connected_tx)));

    let connection = connection_builder
        .on_connect(move |_ctx, identity, token| {
            info!("Connected to SpacetimeDB with identity: {:?}", identity);
            *token_clone.lock().unwrap() = Some(token.to_string());
        })
        .on_disconnect(|_ctx, _err| {
            warn!("Disconnected from SpacetimeDB");
        })
        .on_connect_error(|_ctx, err| {
            error!("Connection error: {:?}", err);
        })
        .build()?;

    let token_for_subscribe = connection_token.clone();
    let tx_for_subscribe = tx.clone();
    let connected_tx_for_applied = connected_tx.clone();

    // Spawn the background thread for SpacetimeDB connection events
    let run_thread = connection.run_threaded();

    // Wait until we are connected and have our token/identity
    loop {
        if connection_token.lock().unwrap().is_some() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    let identity = connection.try_identity().unwrap();

    subscribe_to_spacetime_tables(
        &connection,
        identity,
        token_for_subscribe,
        tx_for_subscribe,
        connected_tx_for_applied,
    );

    // Block until the initial subscription is applied
    connected_rx.await?;

    // Now that the cache is populated, hook up update notifications to wake the main loop
    setup_update_notifications(&connection, tx.clone());

    // Keep the thread handle alive
    std::mem::forget(run_thread);

    Ok(connection)
}

fn subscribe_to_spacetime_tables(
    connection: &DbConnection,
    identity: spacetimedb_sdk::Identity,
    connection_token: Arc<std::sync::Mutex<Option<String>>>,
    tx: tokio::sync::mpsc::UnboundedSender<Event>,
    connected_tx: Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
) {
    let token = connection_token.lock().unwrap().clone().unwrap();

    connection
        .subscription_builder()
        .on_applied(move |ctx| {
            info!("Subscriptions applied. Verifying admin permissions...");

            let has_admin_permissions = ctx
                .db
                .visible_admin_identities()
                .iter()
                .any(|admin| admin.identity == identity);

            if !has_admin_permissions {
                let msg = format!(
                    "Sender identity does not have the required admin permissions.\nAdd this identity ({:?}) to admin_identities via the admin interface and add the token ({}) to the .env/.env.webhook-proxy and restart the sender.",
                    identity, token
                );
                error!("{}", msg);
                let _ = tx.send(Event::FatalError(msg));
            }

            if let Some(chan) = connected_tx.lock().unwrap().take() {
                let _ = chan.send(());
            }
        })
        .subscribe([
            "SELECT * FROM sender_mail_ingress",
            "SELECT * FROM sender_mail_delivery_pending",
            "SELECT * FROM sender_mail_delivery_claimed",
            "SELECT * FROM sender_mail_delivery_messages",
            "SELECT * FROM sender_mail_messages",
            "SELECT * FROM active_subscriptions",
            "SELECT * FROM visible_message_categories",
            "SELECT * FROM visible_category_app_passwords",
            "SELECT * FROM active_unsubscribe_tokens",
            "SELECT * FROM visible_admin_identities",
        ]);
}

fn setup_update_notifications(
    connection: &DbConnection,
    tx: tokio::sync::mpsc::UnboundedSender<Event>,
) {
    trace!("Setting up update notifications...");

    // Wake the loop on ingress row inserts and updates
    {
        let tx = tx.clone();
        connection
            .db
            .sender_mail_ingress()
            .on_insert(move |_ctx, _row| {
                trace!("Ingress row inserted");
                let _ = tx.send(Event::Wakeup);
            });
    }
    {
        let tx = tx.clone();
        connection
            .db
            .sender_mail_ingress()
            .on_update(move |_ctx, _old, _new| {
                trace!("Ingress row updated");
                let _ = tx.send(Event::Wakeup);
            });
    }

    // Wake the loop on pending deliveries
    {
        let tx = tx.clone();
        connection
            .db
            .sender_mail_delivery_pending()
            .on_insert(move |_ctx, _row| {
                trace!("Pending delivery row inserted");
                let _ = tx.send(Event::Wakeup);
            });
    }
    {
        let tx = tx.clone();
        connection
            .db
            .sender_mail_delivery_pending()
            .on_update(move |_ctx, _old, _new| {
                trace!("Pending delivery row updated");
                let _ = tx.send(Event::Wakeup);
            });
    }

    // Wake the loop when a delivery is claimed (inserted into MailDeliveryClaimed)
    {
        let tx = tx.clone();
        connection
            .db
            .sender_mail_delivery_claimed()
            .on_insert(move |_ctx, _row| {
                trace!("Delivery claimed");
                let _ = tx.send(Event::Wakeup);
            });
    }

    // Wake the loop when a claimed delivery is resolved (deleted from MailDeliveryClaimed)
    {
        let tx = tx.clone();
        connection
            .db
            .sender_mail_delivery_claimed()
            .on_delete(move |_ctx, _row| {
                trace!("Claimed delivery resolved");
                let _ = tx.send(Event::Wakeup);
            });
    }

    // Wake the loop on unsubscribe token creation
    {
        let tx = tx.clone();
        connection
            .db
            .active_unsubscribe_tokens()
            .on_insert(move |_ctx, _row| {
                trace!("Unsubscribe token created");
                let _ = tx.send(Event::Wakeup);
            });
    }
}

#[instrument(skip(connection, config), fields(ingress_id = tracing::field::Empty, ingress_job = tracing::field::Empty))]
fn process_fanout_jobs(
    connection: &DbConnection,
    config: &SenderConfig,
    instance_id: &str,
) -> Result<bool, Box<dyn Error>> {
    trace!("process_fanout_jobs checking for work");

    let owner = connection.try_identity().ok_or_else(|| {
        error!("Identity check failed");
        "Identity check failed"
    })?;

    let owned_jobs = self_owned_ingress_jobs(connection, owner, instance_id);

    if owned_jobs.is_empty() {
        trace!("No owned ingress jobs. Requesting next ingress job.");
        if let Err(error) = connection
            .reducers()
            .claim_next_mail_ingress(instance_id.to_string())
        {
            warn!("claim_next_mail_ingress failed: {:?}", error);
        }
        // Return false: no work was done on this tick
        Ok(false)
    } else {
        for job in owned_jobs {
            info!("Processing ingress job: {}", job.id);
            if let Err(error) = process_ingress_job(connection, config, job.clone(), instance_id) {
                let _ = connection.reducers().retry_mail_ingress(
                    job.id.clone(),
                    instance_id.to_string(),
                    error.to_string(),
                );
            }
        }
        // Return true: work was done, notify main loop to check again immediately
        Ok(true)
    }
}

#[instrument(skip(connection))]
fn self_owned_ingress_jobs(
    connection: &DbConnection,
    owner: spacetimedb_sdk::Identity,
    instance_id: &str,
) -> Vec<MailIngress> {
    trace!("getting self-owned ingress jobs");
    connection
        .db
        .sender_mail_ingress()
        .iter()
        .filter(|row| {
            row.claim.claim_owner == Some(owner)
                && row.claim.instance_id.as_deref() == Some(instance_id)
        })
        .collect()
}

enum SubscriptionJobOutcome {
    DeliveryQueued,
    AlreadyQueued,
    AwaitingToken,
}

/// Process a single subscription job for a given mail ingress and message. Do not reprocess when already queued or sent. If the subscription does not have an unsubscribe token, request one and return `AwaitingToken`.
#[instrument(skip(connection, config, ingress, message, category), fields(subscription_id = %subscription.id, subscription_job = true))]
fn process_subscription_job(
    connection: &DbConnection,
    config: &SenderConfig,
    ingress: &MailIngress,
    message: &MailMessage,
    category: &MessageCategory,
    subscription: Subscription,
) -> Result<SubscriptionJobOutcome, Box<dyn Error>> {
    let delivery_id = format!(
        "{}:{}:{}",
        ingress.id, subscription.id, subscription.subscriber_email
    );
    trace!(
        "processing subscription job for delivery_id: {}",
        delivery_id
    );
    if connection
        .db
        .sender_mail_delivery_pending()
        .delivery_id()
        .find(&delivery_id)
        .is_some()
        || connection
            .db
            .sender_mail_delivery_claimed()
            .delivery_id()
            .find(&delivery_id)
            .is_some()
        || connection
            .db
            .sender_mail_delivery_done()
            .delivery_id()
            .find(&delivery_id)
            .is_some()
    {
        trace!("delivery already queued: {}", delivery_id);

        return Ok(SubscriptionJobOutcome::AlreadyQueued);
    }

    let token_row = connection
        .db
        .active_unsubscribe_tokens()
        .iter()
        .find(|t| t.subscription_id == subscription.id);

    let token_row = match token_row {
        Some(row) => row,
        None => {
            info!("Requesting token for {}", subscription.subscriber_email);
            connection
                .reducers()
                .ensure_subscription_unsubscribe_token(subscription.id)?;
            return Ok(SubscriptionJobOutcome::AwaitingToken);
        }
    };

    trace!("composing delivery for subscription: {}", subscription.id);
    let (_headers_raw, raw_message) = compose_delivery(
        config,
        &ingress.id,
        message,
        &subscription,
        category,
        &token_row,
    )?;

    trace!("enqueueing delivery for subscription: {}", subscription.id);
    connection.reducers().enqueue_mail_delivery(
        ingress.id.clone(),
        subscription.id,
        subscription.subscriber_email.clone(),
        Some(subscription.subscriber_account_id),
        category.email_address.clone(),
        message.sender_email.clone(),
        raw_message,
    )?;

    Ok(SubscriptionJobOutcome::DeliveryQueued)
}

#[instrument(skip(connection, config), fields(ingress_id = %ingress.id))]
fn process_ingress_job(
    connection: &DbConnection,
    config: &SenderConfig,
    ingress: MailIngress,
    instance_id: &str,
) -> Result<(), Box<dyn Error>> {
    trace!("processing ingress job for ingress_id: {}", ingress.id);
    // Lookup the mail message
    let message = connection
        .db
        .sender_mail_messages()
        .id()
        .find(&ingress.mail_message_id)
        .ok_or_else(|| format!("MailMessage {} not in local cache", ingress.mail_message_id))?;
    // Lookup the category
    let category = match connection
        .db
        .visible_message_categories()
        .iter()
        .find(|category| category.id == ingress.category_id)
    {
        Some(category) => {
            trace!("Category found {category:?}");
            category
        }
        None => {
            trace!(
                "Category not found for category_id: {}",
                ingress.category_id
            );
            let _ = connection.reducers().fail_mail_ingress(
                ingress.id.clone(),
                instance_id.to_string(),
                "missing message category".to_string(),
            );
            return Err("Missing message category".into());
        }
    };

    // Find all subscriptions for the category
    let mut subscribers: Vec<Subscription> = connection
        .db
        .active_subscriptions()
        .iter()
        .filter(|row| row.category_id == ingress.category_id && is_active_subscription(&row.status))
        .collect();

    trace!("Subscribers found {}", subscribers.len());
    subscribers.sort_by(|left, right| left.subscriber_email.cmp(&right.subscriber_email));
    subscribers.dedup_by(|left, right| left.subscriber_email == right.subscriber_email);

    let mut queued_deliveries = 0;
    let mut awaiting_tokens = 0;

    for subscription in subscribers {
        trace!(
            "processing subscription: {} {}",
            subscription.id,
            subscription.subscriber_email
        );
        match process_subscription_job(
            connection,
            config,
            &ingress,
            &message,
            &category,
            subscription,
        )? {
            SubscriptionJobOutcome::DeliveryQueued => {
                let _ = connection.reducers().increment_mail_ingress_delivery_count(
                    ingress.id.clone(),
                    instance_id.to_string(),
                );
                queued_deliveries += 1;
            }
            SubscriptionJobOutcome::AlreadyQueued => {
                queued_deliveries += 1;
            }
            SubscriptionJobOutcome::AwaitingToken => {
                awaiting_tokens += 1;
            }
        }
    }

    if awaiting_tokens > 0 {
        trace!(
            "Ingress {}: waiting for {} unsubscribe tokens to be generated ({} deliveries already queued)",
            ingress.id, awaiting_tokens, queued_deliveries
        );
        // Do NOT fail the ingress - return an error so the caller leaves it in `processing`
        // status. The main loop will retry it on the next tick, or when an `active_unsubscribe_tokens`
        // insert notification wakes the loop.
        return Err("Waiting for unsubscribe token to be generated".into());
    }

    info!(
        "Ingress {}: all {} deliveries queued, completing ingress job",
        ingress.id, queued_deliveries
    );
    connection
        .reducers()
        .complete_mail_ingress(ingress.id.clone(), instance_id.to_string())?;

    Ok(())
}

fn send_delivery_jobs(
    connection: &DbConnection,
    config: &SenderConfig,
    instance_id: &str,
) -> Result<(), Box<dyn Error>> {
    let owner = match connection.try_identity() {
        Some(identity) => {
            trace!("Succeeded Identity check");
            identity
        }
        None => {
            error!("No identity set!");
            return Err("No identity set".into());
        }
    };
    let owned_jobs = self_owned_delivery_jobs(connection, owner, instance_id);

    for delivery in owned_jobs {
        trace!("processing delivery: {}", delivery.delivery_id);
        let delivery_id = delivery.delivery_id.clone();
        match send_delivery(connection, config, delivery, instance_id) {
            Err(error) => {
                warn!("delivery failure: {}", error);
            }
            Ok(_) => trace!("Delivered Mail {}", delivery_id),
        }
    }
    trace!("processed all claimed delivery jobs");

    Ok(())
}

#[instrument(skip_all)]
fn self_owned_delivery_jobs(
    connection: &DbConnection,
    owner: spacetimedb_sdk::Identity,
    instance_id: &str,
) -> Vec<MailDeliveryClaimed> {
    trace!("fetching self-owned delivery jobs");
    connection
        .db
        .sender_mail_delivery_claimed()
        .iter()
        .filter(|row| row.worker == owner && row.instance_id == instance_id)
        .collect()
}

#[instrument(skip(connection, config), fields(delivery_id = %claimed.delivery_id))]
fn send_delivery(
    connection: &DbConnection,
    config: &SenderConfig,
    claimed: MailDeliveryClaimed,
    instance_id: &str,
) -> Result<(), Box<dyn Error>> {
    trace!("sending delivery: {}", claimed.delivery_id);
    use lettre::address::Envelope;

    let delivery_message = connection
        .db
        .sender_mail_delivery_messages()
        .delivery_id()
        .find(&claimed.delivery_id)
        .ok_or_else(|| {
            format!(
                "MailDeliveryMessage {} not in local cache",
                claimed.delivery_id
            )
        })?;

    let ingress = connection
        .db
        .sender_mail_ingress()
        .id()
        .find(&claimed.ingress_id)
        .ok_or_else(|| format!("MailIngress {} not in local cache", claimed.ingress_id))?;

    let (smtp_username, smtp_password) =
        match resolve_category_smtp_credentials(connection, ingress.category_id) {
            Ok(credentials) => credentials,
            Err(error) => {
                let response = format!("Pre-SMTP error: {error}");
                connection.reducers().fail_mail_delivery(
                    claimed.delivery_id.clone(),
                    instance_id.to_string(),
                    Some(0),
                    response,
                    "missing-category-smtp-credentials".to_string(),
                )?;
                return Err(error);
            }
        };

    let transport = match build_transport(config, &smtp_username, &smtp_password) {
        Ok(transport) => transport,
        Err(error) => {
            let response = format!("Pre-SMTP error: {error}");
            connection.reducers().fail_mail_delivery(
                claimed.delivery_id.clone(),
                instance_id.to_string(),
                Some(0),
                response,
                "smtp-transport-build".to_string(),
            )?;
            return Err(error);
        }
    };

    let envelope_result = {
        let from = ingress.category_email.parse()?;
        let to = vec![delivery_message.recipient_email.parse()?];
        Ok(Envelope::new(Some(from), to)?)
    };

    let envelope = match envelope_result {
        Ok(e) => {
            trace!("envelope: {e:?}");
            e
        }
        Err(error) => {
            trace!("envelope error: {error}");
            let response = format!("Pre-SMTP error: {error}");
            connection.reducers().fail_mail_delivery(
                claimed.delivery_id.clone(),
                instance_id.to_string(),
                Some(0),
                response,
                "pre-smtp".to_string(),
            )?;
            return Err(error);
        }
    };

    match transport.send_raw(&envelope, delivery_message.raw_message.as_bytes()) {
        Ok(response) => {
            let code = response.code().to_string().parse::<u16>().ok();
            info!(
                "Successfully sent delivery {}: {:?}",
                claimed.delivery_id, response
            );
            connection.reducers().mark_mail_delivery_sent(
                claimed.delivery_id.clone(),
                instance_id.to_string(),
                code,
                format!("{response:?}"),
            )?;
            Ok(())
        }
        Err(error) => {
            trace!("send_raw error: {error}");
            let code = error
                .status()
                .map(|status| status.to_string().parse::<u16>().unwrap_or(0));
            let response = error.to_string();
            warn!(
                "Failed to send delivery {}: {}",
                claimed.delivery_id, response
            );
            if is_permanent_error(&error) {
                connection.reducers().fail_mail_delivery(
                    claimed.delivery_id.clone(),
                    instance_id.to_string(),
                    code,
                    response,
                    "smtp-permanent".to_string(),
                )?;
            } else if is_transient_error(&error) {
                trace!("transient error: {error}. Moving to temporary failed table.");
                let delay_micros = 5 * 60 * 1_000_000;
                connection.reducers().schedule_mail_delivery_retry(
                    claimed.delivery_id.clone(),
                    instance_id.to_string(),
                    response,
                    delay_micros,
                )?;
            } else {
                trace!("unknown error: {error}. Moving to temporary failed table.");
                let delay_micros = 5 * 60 * 1_000_000;
                connection.reducers().schedule_mail_delivery_retry(
                    claimed.delivery_id.clone(),
                    instance_id.to_string(),
                    response,
                    delay_micros,
                )?;
            }
            Err(error.into())
        }
    }
}
