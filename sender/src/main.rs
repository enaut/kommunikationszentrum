mod config;
mod mail;
mod module_bindings;

use config::SenderConfig;
use lettre::Transport;
use mail::{build_transport, compose_delivery, is_permanent_error, is_transient_error, resolve_category_smtp_credentials};
use module_bindings::{
    claim_next_mail_delivery, claim_next_mail_ingress, complete_mail_ingress,
    enqueue_mail_delivery, ensure_subscription_unsubscribe_token, fail_mail_delivery,
    fail_mail_ingress, mark_mail_delivery_sent, retry_mail_ingress, schedule_mail_delivery_retry,
    DbConnection, MailDeliveryClaimed, MailIngress, MailMessage, MessageCategory, Subscription,
};
use spacetimedb_sdk::{DbContext, Table, TableWithPrimaryKey as _};
use std::{error::Error, sync::Arc};
use uuid::Uuid;

use crate::module_bindings::{
    increment_mail_ingress_delivery_count, increment_mail_ingress_failed_delivery_count,
    ActiveSubscriptionsTableAccess as _, ActiveUnsubscribeTokensTableAccess as _,
    SenderMailDeliveryClaimedTableAccess as _, SenderMailDeliveryDoneTableAccess as _,
    SenderMailDeliveryPendingTableAccess as _, SenderMailIngressTableAccess as _,
    SenderMailMessagesTableAccess as _, VisibleAdminIdentitiesTableAccess as _,
    VisibleCategoryAppPasswordsTableAccess as _, VisibleMessageCategoriesTableAccess as _,
};
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig as _;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing::{error, info, instrument, trace, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Events sent through the main channel.
enum Event {
    /// Wake the processing loop to check for new work.
    Wakeup,
    /// A fatal error occurred; the main loop should exit with this message.
    FatalError(String),
}

struct OTelProviders {
    tracer_provider: SdkTracerProvider,
    logger_provider: SdkLoggerProvider,
}

/// Initializes OpenTelemetry tracing and logging with OTLP exporters for both spans and logs.
fn init_tracing(config: &SenderConfig) -> OTelProviders {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let resource = Resource::builder()
        .with_attributes(vec![
            KeyValue::new("service.name", "sender"),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ])
        .build();

    // Tracing / span setup
    let span_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.otlp_endpoint)
        .build()
        .expect("Failed to build OTLP span exporter");

    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_resource(resource.clone())
        .build();

    global::set_tracer_provider(tracer_provider.clone());

    let tracer = tracer_provider.tracer("sender");
    let telemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_location(true)
        .with_tracked_inactivity(true);

    // Log export setup: bridge tracing log events → OTLP logs → Alloy → Loki
    let log_exporter = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .with_endpoint(&config.otlp_endpoint)
        .build()
        .expect("Failed to build OTLP log exporter");

    let logger_provider = SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .with_resource(resource.clone())
        .build();

    let log_bridge =
        opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&logger_provider);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive("sender=info".parse().unwrap())
                .from_env_lossy(),
        )
        .with(telemetry_layer)
        .with(log_bridge)
        .with(tracing_subscriber::fmt::layer())
        .init();

    OTelProviders {
        tracer_provider,
        logger_provider,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let instance_id = Uuid::new_v4().to_string();

    // Initialize configuration and tracing/logging
    let config = SenderConfig::from_env();

    let otel_providers = init_tracing(&config);

    info!(
        event = "service_startup",
        instance_id = %instance_id,
        "Starting sender service"
    );

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let connection = Arc::new(connect_to_spacetimedb(&config, tx.clone())?);

    // Setup DB update notifications and subscriptions are registered in the
    // SpacetimeDB connection callback, once the identity is known and the
    // connection has been established.

    // Drive SpacetimeDB connection asynchronously in a background task
    let db_conn = connection.clone();
    let mut pump_handle = tokio::spawn(async move { db_conn.run_async().await });

    // Build the SMTP transport for sending emails
    info!("Entering mail processing loop. Press Ctrl+C to stop.");

    // Setup a shutdown signal listener for graceful termination
    let shutdown_signal = tokio::signal::ctrl_c();
    tokio::pin!(shutdown_signal);

    // Bootstrap: wake the loop once immediately so it checks for work on startup.
    let _ = tx.send(Event::Wakeup);

    // Main reactive processing loop: wait for database updates, shutdown signal, or pump termination
    loop {
        tokio::select! {
            _ = &mut shutdown_signal => {
                info!("Shutdown signal received … exiting processing loop …");
                break;
            }

            pump_res = &mut pump_handle => {
                let err_msg = match pump_res {
                    Ok(Err(err)) => {
                        format!("SpacetimeDB async pump terminated unexpectedly: {:?}", err)
                    }
                    Ok(Ok(())) => {
                        format!("SpacetimeDB async pump terminated unexpectedly")
                    }
                    Err(join_err) => {
                        format!("SpacetimeDB async pump task join error: {:?}", join_err)
                    }
                };
                error!("{}", err_msg);
                return Err(err_msg.into());
            }

            Some(event) = rx.recv() => {
                match event {
                    Event::FatalError(msg) => {
                        error!("{}", msg);
                        break;
                    }
                    Event::Wakeup => {
                        trace!("Database subscription updated. Processing jobs...");

                        // Process fanout jobs take one incoming mail and create delivery jobs for all subscribers.
                        match process_fanout_jobs(&connection, &config, &instance_id) {
                            Ok(_) => (),
                            Err(error) => error!("Error processing fanout jobs: {:?}", error),
                        };

                        // Process delivery jobs take one delivery job and send it via SMTP.
                        match claim_delivery_jobs(&connection, &config, &instance_id){
                            Ok(_) => (),
                            Err(error) => error!("Error processing delivery jobs: {:?}", error),
                        }
                    }
                }
            }
        }
    }

    // Graceful shutdown: abort the SpacetimeDB pump and shutdown tracing/logging
    pump_handle.abort();
    info!("Shutting down tracing and logging...");
    otel_providers.tracer_provider.shutdown()?;
    otel_providers.logger_provider.shutdown()?;
    info!("Sender service stopped.");
    Ok(())
}

fn connect_to_spacetimedb(
    config: &SenderConfig,
    tx: tokio::sync::mpsc::UnboundedSender<Event>,
) -> Result<DbConnection, Box<dyn Error>> {
    trace!("Connecting to SpacetimeDB...");
    info!("Config: {:?}", config);

    let mut builder = DbConnection::builder()
        .with_uri(config.spacetimedb_uri.clone())
        .with_database_name(config.spacetimedb_database_name.clone());
    let connection_token = Arc::new(std::sync::Mutex::new(None));
    let token_for_connect = connection_token.clone();
    let token_for_subscribe = connection_token.clone();

    if let Some(token) = &config.spacetimedb_token {
        builder = builder.with_token(Some(token.clone()));
    }

    builder = builder.on_connect(move |connection, identity, token| {
        info!(
            "Successfully connected to SpacetimeDB with identity: {:?}",
            identity
        );
        if let Ok(mut lock) = token_for_connect.lock() {
            *lock = Some(token.to_string());
        }
        subscribe_to_spacetime_tables(
            connection,
            token_for_subscribe
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .to_owned(),
            tx.clone(),
        );
        setup_update_notifications(connection, tx.clone());
    });

    Ok(builder.build()?)
}

fn subscribe_to_spacetime_tables(
    connection: &DbConnection,
    token: String,
    tx: tokio::sync::mpsc::UnboundedSender<Event>,
) {
    trace!("Subscribing to SpacetimeDB tables...");

    let identity = connection.identity();
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
        })
        .subscribe([
            "SELECT * FROM sender_mail_ingress",
            "SELECT * FROM sender_mail_delivery_pending",
            "SELECT * FROM sender_mail_delivery_claimed",
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
        .id()
        .find(&delivery_id)
        .is_some()
        || connection
            .db
            .sender_mail_delivery_claimed()
            .id()
            .find(&delivery_id)
            .is_some()
        || connection
            .db
            .sender_mail_delivery_done()
            .id()
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
        category.name.clone(),
        message.sender_email.clone(),
        category.email_address.clone(),
        message.sender_email.clone(),
        raw_message,
        token_row.token.clone(),
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
            return Ok(());
        }
    };

    let Some(app_password_id) = category.app_password_id else {
        let _ = connection.reducers().fail_mail_ingress(
            ingress.id.clone(),
            instance_id.to_string(),
            "category has no SMTP app password".to_string(),
        );
        return Ok(());
    };

    if connection
        .db
        .visible_category_app_passwords()
        .id()
        .find(&app_password_id)
        .is_none()
    {
        return Err(format!(
            "SMTP app password for category {} not in local cache yet",
            category.id
        )
        .into());
    }

    let mut subscribers: Vec<Subscription> = connection
        .db
        .active_subscriptions()
        .iter()
        .filter(|sub| sub.category_id == ingress.category_id)
        .collect();

    subscribers.sort_by(|left, right| left.subscriber_email.cmp(&right.subscriber_email));
    subscribers.dedup_by(|left, right| left.subscriber_email == right.subscriber_email);

    let mut waiting_for_tokens = false;

    for subscription in subscribers {
        trace!("processing subscription: {}", subscription.id);
        let subscription_outcome = process_subscription_job(
            connection,
            config,
            &ingress,
            &message,
            &category,
            subscription.clone(),
        )
        .map_err(|e| {
            trace!("error processing subscription: {}", e);
            // Increment the failed delivery count for this ingress.
            let _ = connection
                .reducers()
                .increment_mail_ingress_failed_delivery_count(
                    ingress.id.clone(),
                    instance_id.to_string(),
                );
            e
        })?;
        match subscription_outcome {
            SubscriptionJobOutcome::DeliveryQueued => {
                trace!("delivery queued for subscription: {}", subscription.id);
                connection
                    .reducers()
                    .increment_mail_ingress_delivery_count(
                        ingress.id.clone(),
                        instance_id.to_string(),
                    )?;
            }
            SubscriptionJobOutcome::AlreadyQueued => {
                trace!(
                    "delivery already queued for subscription: {}",
                    subscription.id
                );
                // No action needed; delivery already queued or sent
            }
            SubscriptionJobOutcome::AwaitingToken => {
                trace!("awaiting token for subscription: {}", subscription.id);
                waiting_for_tokens = true;
            }
        }
    }

    if waiting_for_tokens {
        trace!("waiting for tokens");
        return Err("Waiting for unsubscribe token to be generated".into());
    }

    connection
        .reducers()
        .complete_mail_ingress(ingress.id.clone(), instance_id.to_string())?;
    Ok(())
}

#[instrument(skip_all)]
fn claim_delivery_jobs(
    connection: &DbConnection, // No longer needs Arc!
    config: &SenderConfig,
    instance_id: &str,
) -> Result<(), Box<dyn Error>> {
    trace!("claiming delivery jobs");
    let owner = connection.try_identity().ok_or_else(|| {
        error!("No identity set!");
        "No identity set"
    })?;

    let owned_jobs = self_owned_delivery_jobs(connection, owner, instance_id);
    trace!("owned jobs: {}", owned_jobs.len());

    if owned_jobs.is_empty() {
        trace!("no owned jobs, claiming next mail delivery via transaction");
        // Fire and forget. When the database updates, your `on_insert`
        // handler will automatically wake the main loop up.
        if let Err(error) = connection
            .reducers()
            .claim_next_mail_delivery(instance_id.to_owned())
        {
            warn!("claim_next_mail_delivery reducer call failed: {error:?}");
        }
    } else {
        trace!("processing claimed delivery jobs");
        process_claimed_delivery_jobs(connection, config, instance_id)?;
    }

    Ok(())
}

#[instrument(skip_all)]
fn process_claimed_delivery_jobs(
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
        trace!("processing delivery: {}", delivery.id);
        let delivery_id = delivery.id.clone();
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

#[instrument(skip(connection, config), fields(delivery_id = %claimed.id))]
fn send_delivery(
    connection: &DbConnection,
    config: &SenderConfig,
    claimed: MailDeliveryClaimed,
    instance_id: &str,
) -> Result<(), Box<dyn Error>> {
    trace!("sending delivery: {}", claimed.id);
    use lettre::address::Envelope;

    let (smtp_username, smtp_password) =
        match resolve_category_smtp_credentials(connection, claimed.row.category_id) {
            Ok(credentials) => credentials,
            Err(error) => {
                let response = format!("Pre-SMTP error: {error}");
                connection.reducers().fail_mail_delivery(
                    claimed.id.clone(),
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
                claimed.id.clone(),
                instance_id.to_string(),
                Some(0),
                response,
                "smtp-transport-build".to_string(),
            )?;
            return Err(error);
        }
    };

    let envelope_result = {
        let from = claimed.row.original_sender_email.parse()?;
        let to = vec![claimed.row.recipient_email.parse()?];
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
                claimed.id.clone(),
                instance_id.to_string(),
                Some(0),
                response,
                "pre-smtp".to_string(),
            )?;
            return Err(error);
        }
    };

    match transport.send_raw(&envelope, claimed.row.raw_message.as_bytes()) {
        Ok(response) => {
            let code = response.code().to_string().parse::<u16>().ok();
            info!("Successfully sent delivery {}: {:?}", claimed.id, response);
            connection.reducers().mark_mail_delivery_sent(
                claimed.id.clone(),
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
            warn!("Failed to send delivery {}: {}", claimed.id, response);
            if is_permanent_error(&error) {
                connection.reducers().fail_mail_delivery(
                    claimed.id.clone(),
                    instance_id.to_string(),
                    code,
                    response,
                    "smtp-permanent".to_string(),
                )?;
            } else if is_transient_error(&error) {
                trace!("transient error: {error}. Moving to temporary failed table.");
                let delay_micros = 5 * 60 * 1_000_000;
                connection.reducers().schedule_mail_delivery_retry(
                    claimed.id.clone(),
                    response,
                    delay_micros,
                )?;
            } else {
                trace!("unknown error: {error}. Moving to temporary failed table.");
                let delay_micros = 5 * 60 * 1_000_000;
                connection.reducers().schedule_mail_delivery_retry(
                    claimed.id.clone(),
                    response,
                    delay_micros,
                )?;
            }
            Err(error.into())
        }
    }
}
