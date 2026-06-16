use chrono::Utc;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::Error as SmtpError;
use lettre::SmtpTransport;
use serde_json::to_string;
use std::error::Error;

use crate::config::SenderConfig;
use crate::module_bindings::{
    MailIngress, MessageCategory, Subscription, SubscriptionUnsubscribeToken,
};

pub fn build_transport(config: &SenderConfig) -> Result<SmtpTransport, Box<dyn Error>> {
    let mut builder = if config.smtp_use_tls {
        SmtpTransport::relay(&config.smtp_host)?
    } else {
        SmtpTransport::builder_dangerous(&config.smtp_host)
    };

    builder = builder.port(config.smtp_port);

    if let (Some(username), Some(password)) = (&config.smtp_username, &config.smtp_password) {
        builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
    }

    Ok(builder.build())
}

pub fn is_transient_error(error: &SmtpError) -> bool {
    error.is_transient() || error.is_timeout() || error.is_response()
}

pub fn is_permanent_error(error: &SmtpError) -> bool {
    error.is_permanent()
}

pub fn compose_delivery(
    config: &SenderConfig,
    ingress: &MailIngress,
    subscription: &Subscription,
    category: &MessageCategory,
    token: &SubscriptionUnsubscribeToken,
) -> Result<(String, String), Box<dyn Error>> {
    let list_email = category.email_address.clone();
    let list_name = if category.name.trim().is_empty() {
        category
            .email_address
            .split('@')
            .next()
            .unwrap_or("list")
            .to_string()
    } else {
        category.name.clone()
    };
    let recipient_email = subscription.subscriber_email.clone();
    let subject = rewrite_subject(&list_name, &ingress.subject);
    let reply_to = ingress.sender_email.clone();
    let message_id = format!(
        "<{}@{}>",
        message_id_seed(&ingress.id, &recipient_email),
        config.message_id_domain
    );
    let date = Utc::now().to_rfc2822();
    let unsubscribe_url = format!("{}?token={}", config.unsubscribe_base_url, token.token);

    let headers = vec![
        ("From".to_string(), list_email.clone()),
        ("To".to_string(), recipient_email.clone()),
        ("Reply-To".to_string(), reply_to.clone()),
        ("Subject".to_string(), subject.clone()),
        ("Message-ID".to_string(), message_id),
        ("Date".to_string(), date),
        (
            "List-Id".to_string(),
            format!("{} <{}>", list_name, list_email),
        ),
        ("List-Post".to_string(), format!("<mailto:{}>", list_email)),
        (
            "List-Unsubscribe".to_string(),
            format!(
                "<mailto:{}?subject=unsubscribe>, <{}>",
                list_email, unsubscribe_url
            ),
        ),
        (
            "List-Unsubscribe-Post".to_string(),
            "List-Unsubscribe=One-Click".to_string(),
        ),
        ("Precedence".to_string(), "list".to_string()),
        ("Sender".to_string(), list_email.clone()),
        ("X-Mailing-List".to_string(), list_name.clone()),
        ("X-BeenThere".to_string(), list_email.clone()),
    ];

    let headers_raw = to_string(&headers)?;
    let raw_message = render_raw_message(&headers, &ingress.body_raw);
    Ok((headers_raw, raw_message))
}

fn sanitize_header_value(value: &str) -> String {
    value.replace(['\r', '\n'], "")
}

fn rewrite_subject(list_name: &str, subject: &str) -> String {
    let prefix = format!("{list_name}: ");
    let lower_subject = subject.to_ascii_lowercase();
    let lower_prefix = prefix.to_ascii_lowercase();

    if lower_subject.starts_with(&lower_prefix) {
        subject.to_string()
    } else {
        format!("{prefix}{subject}")
    }
}

fn render_raw_message(headers: &[(String, String)], body: &str) -> String {
    let mut raw = String::new();
    for (name, value) in headers {
        raw.push_str(&sanitize_header_value(name));
        raw.push_str(": ");
        raw.push_str(&sanitize_header_value(value));
        raw.push_str("\r\n");
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    raw
}

fn message_id_seed(ingress_id: &str, recipient_email: &str) -> String {
    format!(
        "{}-{}",
        ingress_id.replace(':', "-"),
        recipient_email.replace('@', "-at-")
    )
}
