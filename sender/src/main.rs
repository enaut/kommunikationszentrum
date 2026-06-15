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
use std::{collections::HashSet, error::Error, thread, time::Duration};

use crate::module_bindings::{
    ActiveSubscriptionsTableAccess as _, ActiveUnsubscribeTokensTableAccess as _,
    MessageCategoriesTableAccess as _, SenderMailDeliveriesTableAccess as _,
    SenderMailIngressTableAccess as _,
};

const INGESTED_STATE: &str = "processing";
const DELIVERY_STATE: &str = "sending";

fn main() -> Result<(), Box<dyn Error>> {
    let config = SenderConfig::from_env();
    let connection = connect(&config)?;
    subscribe(&connection);
    let _pump = connection.run_threaded();

    let transport = build_transport(&config)?;
    println!("sender connected as {:?}", connection.try_identity());

    loop {
        let mut did_work = false;
        did_work |= process_fanout_jobs(&connection, &config)?;
        did_work |= process_delivery_jobs(&connection, &transport)?;

        if !did_work {
            thread::sleep(config.poll_interval);
        }
    }
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

fn process_fanout_jobs(
    connection: &DbConnection,
    config: &SenderConfig,
) -> Result<bool, Box<dyn Error>> {
    let owner = match connection.try_identity() {
        Some(identity) => identity,
        None => return Ok(false),
    };

    let mut processed = HashSet::new();
    let mut did_work = false;

    loop {
        let owned_jobs = self_owned_ingress_jobs(connection, owner, &processed);
        if owned_jobs.is_empty() {
            if let Err(error) = connection.reducers().claim_next_mail_ingress() {
                eprintln!("claim_next_mail_ingress failed: {:?}", error);
                break;
            }
            thread::sleep(Duration::from_millis(50));
            let owned_after = self_owned_ingress_jobs(connection, owner, &processed);
            if owned_after.is_empty() {
                break;
            }
            for job in owned_after {
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

    for subscription in subscriptions {
        let token_row = connection
            .db
            .active_unsubscribe_tokens()
            .iter()
            .find(|t| t.subscription_id == subscription.id);

        let token_row = match token_row {
            Some(row) => row,
            None => {
                connection
                    .reducers()
                    .ensure_subscription_unsubscribe_token(subscription.id)?;
                return Err("Waiting for unsubscribe token to be generated".into());
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

    connection.reducers().complete_mail_ingress(
        ingress.id.clone(),
        deliveries_created,
        _deliveries_failed,
    )?;
    Ok(())
}

fn process_delivery_jobs(
    connection: &DbConnection,
    transport: &SmtpTransport,
) -> Result<bool, Box<dyn Error>> {
    let owner = match connection.try_identity() {
        Some(identity) => identity,
        None => return Ok(false),
    };

    let mut processed = HashSet::new();
    let mut did_work = false;

    loop {
        let owned_jobs = self_owned_delivery_jobs(connection, owner, &processed);
        if owned_jobs.is_empty() {
            if let Err(error) = connection.reducers().claim_next_mail_delivery() {
                eprintln!("claim_next_mail_delivery failed: {:?}", error);
                break;
            }
            thread::sleep(Duration::from_millis(50));
            let owned_after = self_owned_delivery_jobs(connection, owner, &processed);
            if owned_after.is_empty() {
                break;
            }
            for delivery in owned_after {
                if let Err(error) = send_delivery(connection, transport, delivery.clone()) {
                    eprintln!("delivery {} failed: {}", delivery.id, error);
                }
                processed.insert(delivery.id);
                did_work = true;
            }
            continue;
        }

        for delivery in owned_jobs {
            if let Err(error) = send_delivery(connection, transport, delivery.clone()) {
                eprintln!("delivery {} failed: {}", delivery.id, error);
            }
            processed.insert(delivery.id);
            did_work = true;
        }
    }

    Ok(did_work)
}

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

fn send_delivery(
    connection: &DbConnection,
    transport: &SmtpTransport,
    delivery: MailDelivery,
) -> Result<(), Box<dyn Error>> {
    use lettre::address::Envelope;

    let envelope = Envelope::new(
        Some(delivery.original_sender_email.parse()?),
        vec![delivery.recipient_email.parse()?],
    )?;

    match transport.send_raw(&envelope, delivery.raw_message.as_bytes()) {
        Ok(response) => {
            let code = response.code().to_string().parse::<u16>().ok();
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
