mod config;
mod mail;
mod module_bindings;

use config::SenderConfig;
use lettre::{SmtpTransport, Transport};
use mail::{build_transport, compose_delivery, is_permanent_error, is_transient_error};
use module_bindings::{
    claim_next_mail_delivery, claim_next_mail_ingress, complete_mail_ingress,
    enqueue_mail_delivery, ensure_subscription_unsubscribe_token, fail_mail_delivery,
    fail_mail_ingress, mark_mail_delivery_sent, retry_mail_ingress, schedule_mail_delivery_retry,
    DbConnection, MailDelivery, MailIngress, Subscription,
};
use spacetimedb_sdk::{DbContext, Table, Timestamp};
use std::{collections::HashSet, error::Error, time::Duration};

use crate::module_bindings::{
    ActiveSubscriptionsTableAccess as _, ActiveUnsubscribeTokensTableAccess as _,
    MessageCategoriesTableAccess as _, SenderMailDeliveriesTableAccess as _,
    SenderMailIngressTableAccess as _,
};
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig as _;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing::{error, info, instrument, trace, warn, Span};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const INGESTED_STATE: &str = "processing";
const DELIVERY_STATE: &str = "sending";

struct OTelProviders {
    tracer_provider: SdkTracerProvider,
    logger_provider: SdkLoggerProvider,
}

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
    let config = SenderConfig::from_env();
    let otel_providers = init_tracing(&config);

    // Test event to verify "dots" in Jaeger
    info!(event = "service_startup", "Starting sender service");

    let connection = connect(&config)?;
    subscribe(&connection);
    let _pump = connection.run_threaded();

    let transport = build_transport(&config)?;
    info!("sender connected as {:?}", connection.try_identity());

    info!("Entering main processing loop. Press Ctrl+C to stop.");

    let shutdown_signal = tokio::signal::ctrl_c();
    tokio::pin!(shutdown_signal);

    loop {
        let mut did_work = false;

        tokio::select! {
            _ = &mut shutdown_signal => {
                info!("Shutdown signal received");
                break;
            }
            fanout_res = process_fanout_jobs(&connection, &config) => {
                trace!("Completed a fanout processing iteration");
                did_work |= fanout_res?;
            }
        }

        tokio::select! {
            _ = &mut shutdown_signal => {
                info!("Shutdown signal received");
                break;
            }
            delivery_res = process_delivery_jobs(&connection, &transport) => {
                trace!("Processed delivery jobs iteration");
                did_work |= delivery_res?;
            }
        }

        if !did_work {
            tokio::select! {
                _ = &mut shutdown_signal => {
                    info!("Shutdown signal received");
                    break;
                }
                _ = tokio::time::sleep(config.poll_interval) => {}
            }
        }
    }

    info!("Shutting down tracing and logging...");
    otel_providers.tracer_provider.shutdown()?;
    otel_providers.logger_provider.shutdown()?;
    info!("Sender service stopped.");
    Ok(())
}

fn connect(config: &SenderConfig) -> Result<DbConnection, Box<dyn Error>> {
    let mut builder = DbConnection::builder()
        .with_uri(config.spacetimedb_uri.clone())
        .with_database_name(config.spacetimedb_database_name.clone());

    if let Some(token) = &config.spacetimedb_token {
        builder = builder.with_token(Some(token.clone()));
    }

    Ok(builder.build()?)
}

fn subscribe(connection: &DbConnection) {
    connection.subscription_builder().subscribe([
        "SELECT * FROM sender_mail_ingress",
        "SELECT * FROM sender_mail_deliveries",
        "SELECT * FROM active_subscriptions",
        "SELECT * FROM message_categories",
        "SELECT * FROM active_unsubscribe_tokens",
    ]);
}

#[instrument(skip_all)]
async fn process_fanout_jobs(
    connection: &DbConnection,
    config: &SenderConfig,
) -> Result<bool, Box<dyn Error>> {
    let owner = match connection.try_identity() {
        Some(identity) => {
            info!("Identity check succeeded");
            identity
        }
        None => {
            error!("Identity check failed");
            return Ok(false);
        }
    };

    let mut processed = HashSet::new();
    let mut did_work = false;

    trace!("Checking for mail ingress jobs owned by this instance");

    loop {
        let owned_jobs = self_owned_ingress_jobs(connection, owner, &processed);
        if owned_jobs.is_empty() {
            if let Err(error) = connection.reducers().claim_next_mail_ingress() {
                warn!("claim_next_mail_ingress failed: {:?}", error);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            let owned_after = self_owned_ingress_jobs(connection, owner, &processed);
            if owned_after.is_empty() {
                trace!("No new mail ingress jobs after waiting");
                break;
            }
            for job in owned_after {
                info!("Processing ingress job: {}", job.id);
                Span::current().record("ingress_id", &job.id.as_str());
                Span::current().record("ingress_job", true);
                if let Err(error) = process_ingress_job(connection, config, job.clone()) {
                    let _ = connection
                        .reducers()
                        .retry_mail_ingress(job.id.clone(), error.to_string());
                }
                processed.insert(job.id);
                did_work = true;
            }
            continue;
        }

        for job in owned_jobs {
            info!("Processing ingress job: {}", job.id);
            if let Err(error) = process_ingress_job(connection, config, job.clone()) {
                let _ = connection
                    .reducers()
                    .retry_mail_ingress(job.id.clone(), error.to_string());
            }
            processed.insert(job.id);
            did_work = true;
        }
    }

    Ok(did_work)
}

#[instrument(skip(connection, processed))]
fn self_owned_ingress_jobs<'a>(
    connection: &'a DbConnection,
    owner: spacetimedb_sdk::Identity,
    processed: &HashSet<String>,
) -> Vec<MailIngress> {
    connection
        .db
        .sender_mail_ingress()
        .iter()
        .filter(|row| row.state == INGESTED_STATE && row.claim_owner == Some(owner))
        .filter(|row| row.completed_at == Timestamp::UNIX_EPOCH)
        .filter(|row| !processed.contains(&row.id))
        .collect()
}

#[instrument(skip(connection, config), fields(ingress_id = %ingress.id))]
fn process_ingress_job(
    connection: &DbConnection,
    config: &SenderConfig,
    ingress: MailIngress,
) -> Result<(), Box<dyn Error>> {
    let category = match connection
        .db
        .message_categories()
        .id()
        .find(&ingress.category_id)
    {
        Some(category) => category,
        None => {
            let _ = connection
                .reducers()
                .fail_mail_ingress(ingress.id.clone(), "missing message category".to_string());
            return Ok(());
        }
    };

    let mut subscriptions: Vec<Subscription> = connection
        .db
        .active_subscriptions()
        .iter()
        .filter(|sub| sub.category_id == ingress.category_id)
        .filter(|sub| sub.active)
        .collect();

    subscriptions.sort_by(|left, right| left.subscriber_email.cmp(&right.subscriber_email));
    subscriptions.dedup_by(|left, right| left.subscriber_email == right.subscriber_email);

    if subscriptions.is_empty() {
        connection
            .reducers()
            .complete_mail_ingress(ingress.id.clone(), 0, 0)?;
        return Ok(());
    }

    let mut deliveries_created = 0u32;
    let _deliveries_failed = 0u32;
    let mut waiting_for_tokens = false;

    for subscription in subscriptions {
        Span::current().record("subscription_id", &subscription.id);
        Span::current().record("subscription_job", true);
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
                waiting_for_tokens = true;
                continue;
            }
        };

        let (headers_raw, raw_message) =
            compose_delivery(config, &ingress, &subscription, &category, &token_row)?;

        connection.reducers().enqueue_mail_delivery(
            ingress.id.clone(),
            subscription.id,
            subscription.subscriber_email.clone(),
            Some(subscription.subscriber_account_id),
            category.email_address.clone(),
            category.name.clone(),
            ingress.sender_email.clone(),
            category.email_address.clone(),
            ingress.sender_email.clone(),
            ingress.subject.clone(),
            ingress.body_raw.clone(),
            headers_raw,
            raw_message,
            token_row.token.clone(),
        )?;
        deliveries_created = deliveries_created.saturating_add(1);
    }

    if waiting_for_tokens {
        return Err("Waiting for unsubscribe token to be generated".into());
    }

    connection.reducers().complete_mail_ingress(
        ingress.id.clone(),
        deliveries_created,
        _deliveries_failed,
    )?;
    Ok(())
}

#[instrument(skip_all)]
async fn process_delivery_jobs(
    connection: &DbConnection,
    transport: &SmtpTransport,
) -> Result<bool, Box<dyn Error>> {
    let owner = match connection.try_identity() {
        Some(identity) => {
            info!("Succeeded Identity check");
            identity
        }
        None => {
            error!("No identity set!");
            return Ok(false);
        }
    };
    let mut processed = HashSet::new();
    let mut did_work = false;

    loop {
        let owned_jobs = self_owned_delivery_jobs(connection, owner, &processed);
        if owned_jobs.is_empty() {
            if let Err(error) = connection.reducers().claim_next_mail_delivery() {
                warn!("claim_next_mail_delivery failed: {:?}", error);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            let owned_after = self_owned_delivery_jobs(connection, owner, &processed);
            if owned_after.is_empty() {
                trace!("No new mail delivery jobs after waiting");
                break;
            }
            for delivery in owned_after {
                if let Err(error) = send_delivery(connection, transport, delivery.clone()) {
                    warn!("delivery {} failed: {}", delivery.id, error);
                }
                processed.insert(delivery.id);
                trace!("Processed successfully");
                did_work = true;
            }
            continue;
        }

        for delivery in owned_jobs {
            if let Err(error) = send_delivery(connection, transport, delivery.clone()) {
                warn!("delivery {} failed: {}", delivery.id, error);
            }
            processed.insert(delivery.id);
            did_work = true;
        }
    }

    Ok(did_work)
}

#[instrument(skip_all)]
fn self_owned_delivery_jobs<'a>(
    connection: &'a DbConnection,
    owner: spacetimedb_sdk::Identity,
    processed: &HashSet<String>,
) -> Vec<MailDelivery> {
    connection
        .db
        .sender_mail_deliveries()
        .iter()
        .filter(|row| row.state == DELIVERY_STATE && row.claim_owner == Some(owner))
        .filter(|row| row.sent_at == Timestamp::UNIX_EPOCH)
        .filter(|row| !processed.contains(&row.id))
        .collect()
}

#[instrument(skip(connection, transport), fields(delivery_id = %delivery.id))]
fn send_delivery(
    connection: &DbConnection,
    transport: &SmtpTransport,
    delivery: MailDelivery,
) -> Result<(), Box<dyn Error>> {
    use lettre::address::Envelope;

    // Capture pre-SMTP errors (parsing, etc.) to ensure we update the state in SpaceTimeDB
    let envelope_result = (|| -> Result<Envelope, Box<dyn Error>> {
        let from = delivery.original_sender_email.parse()?;
        let to = vec![delivery.recipient_email.parse()?];
        Ok(Envelope::new(Some(from), to)?)
    })();

    let envelope = match envelope_result {
        Ok(e) => e,
        Err(error) => {
            let response = format!("Pre-SMTP error: {error}");
            connection.reducers().fail_mail_delivery(
                delivery.id.clone(),
                Some(0),
                response,
                "pre-smtp".to_string(),
            )?;
            return Err(error);
        }
    };

    match transport.send_raw(&envelope, delivery.raw_message.as_bytes()) {
        Ok(response) => {
            let code = response.code().to_string().parse::<u16>().ok();
            info!("Successfully sent delivery {}: {:?}", delivery.id, response);
            connection.reducers().mark_mail_delivery_sent(
                delivery.id.clone(),
                code,
                format!("{response:?}"),
            )?;
        }
        Err(error) => {
            let code = error
                .status()
                .map(|status| status.to_string().parse::<u16>().unwrap_or(0));
            let response = error.to_string();
            warn!("Failed to send delivery {}: {}", delivery.id, response);
            if is_permanent_error(&error) {
                connection.reducers().fail_mail_delivery(
                    delivery.id.clone(),
                    code,
                    response,
                    "smtp-permanent".to_string(),
                )?;
            } else if is_transient_error(&error) {
                connection.reducers().schedule_mail_delivery_retry(
                    delivery.id.clone(),
                    code,
                    response,
                    "smtp-transient".to_string(),
                )?;
            } else {
                connection.reducers().schedule_mail_delivery_retry(
                    delivery.id.clone(),
                    code,
                    response,
                    "smtp-unknown".to_string(),
                )?;
            }
        }
    }

    Ok(())
}
