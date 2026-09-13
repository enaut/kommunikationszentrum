use stalwart_mta_hook_types::Message as MtaMessage;

use crate::services::mta::envelope_parser::extract_header;

/// Reason why an email could not be delivered to a topic category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopicRejectionReason {
    /// The user is subscribed to the topic, but only has Read permission (no Write permission).
    NoWritePermission,
    /// The user has an active account, but has no active subscription to this topic.
    NotSubscribed,
    /// The sender's email address is not associated with any active verified account in the system.
    NotRegistered,
}

/// Information about a topic category that rejected the incoming message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedTopic {
    pub topic_name: String,
    pub topic_email: String,
    pub reason: TopicRejectionReason,
}

/// Checks whether an address is safe to receive automated rejection emails.
/// Prevents bounce loops (e.g. against `<>`, `unknown`, MAILER-DAEMON, postmaster, or noreply addresses).
pub fn is_valid_bounce_recipient(from: &str) -> bool {
    let trimmed = from.trim();
    if trimmed.is_empty()
        || trimmed == "<>"
        || trimmed.eq_ignore_ascii_case("unknown")
        || !trimmed.contains('@')
    {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    let local_part = lower.split('@').next().unwrap_or("");

    if local_part == "mailer-daemon"
        || local_part == "postmaster"
        || local_part == "no-reply"
        || local_part == "noreply"
    {
        return false;
    }

    true
}

/// Builds the subject and body for a rejection notification email to be sent by SMTP_SYSTEM_USER.
pub fn build_rejection_email(
    original_subject: &str,
    message: Option<&MtaMessage>,
    rejected_topics: &[RejectedTopic],
    from_address: &str,
) -> (String, String) {
    let subject_clean = original_subject.trim();
    let subject = if subject_clean.is_empty() || subject_clean.eq_ignore_ascii_case("no subject") {
        let topic_list = rejected_topics
            .iter()
            .map(|t| t.topic_email.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("Rejected: Mail to {topic_list}")
    } else {
        format!("Rejected: {subject_clean}")
    };

    let date_header = message
        .and_then(|m| extract_header(&m.headers, "date"))
        .unwrap_or_else(|| "N/A".to_string());

    let message_id_header = message
        .and_then(|m| extract_header(&m.headers, "message-id"))
        .unwrap_or_else(|| "N/A".to_string());

    let mut body_de = String::new();
    let mut body_en = String::new();

    body_de.push_str("-------------------Deutsch------------------------\n");
    body_en.push_str("-------------------English------------------------\n");
    body_de.push_str("Ihre E-Mail konnte nicht an alle Empfänger zugestellt werden.\n");
    body_en.push_str("Your email could not be delivered to all recipients.\n\n");

    body_de.push_str("--------------------------------------------------\n");
    body_de.push_str("BETROFFENE THEMEN:\n");
    body_de.push_str("--------------------------------------------------\n");
    body_en.push_str("--------------------------------------------------\n");
    body_en.push_str("AFFECTED TOPICS:\n");
    body_en.push_str("--------------------------------------------------\n");

    for topic in rejected_topics {
        let name = if topic.topic_name.trim().is_empty() {
            &topic.topic_email
        } else {
            &topic.topic_name
        };

        body_de.push_str(&format!("• Thema: {} <{}>\n", name, topic.topic_email));
        body_en.push_str(&format!("• Topic: {} <{}>\n", name, topic.topic_email));
        match topic.reason {
            TopicRejectionReason::NoWritePermission => {
                body_de.push_str("  Grund: Sie haben für dieses Thema keine Schreibberechtigung. Ihr Abonnement ist auf 'Nur Lesen' (Read) eingestellt. Bitte wenden Sie sich an die Administration oder passen Sie Ihre Abonnementeinstellungen an, falls Sie Schreibrechte benötigen.\n");
                body_en.push_str("  Reason: You do not have write permission for this topic. Your subscription is set to read-only. Please contact an administrator or update your subscription settings if you need write permissions.\n\n");
            }
            TopicRejectionReason::NotSubscribed => {
                body_de.push_str("  Grund: Sie haben dieses Thema nicht abonniert. Nur abonnierte Mitglieder mit Schreibberechtigung können Nachrichten an dieses Thema senden. Sie können dieses Thema in Ihrem Benutzerprofil abonnieren.\n");
                body_en.push_str("  Reason: You are not subscribed to this topic. Only subscribed members with write permission can post messages to this topic. You can subscribe to this topic in your member profile.\n\n");
            }
            TopicRejectionReason::NotRegistered => {
                body_de.push_str(&format!(
                    "  Grund: Ihre Absenderadresse '{}' ist keinem aktiven Mitgliedskonto im Kommunikationszentrum zugeordnet. Bitte senden Sie E-Mails von Ihrer bei uns registrierten E-Mail-Adresse oder hinterlegen Sie diese Adresse in Ihrem Profil.\n",
                    from_address
                ));
                body_en.push_str(&format!(
                    "  Reason: Your sender email address '{}' is not associated with an active member account in Kommunikationszentrum. Please send emails from your registered email address or add this address to your account profile.\n\n",
                    from_address
                ));
            }
        }
    }

    body_de.push_str("--------------------------------------------------\n");
    body_de.push_str("DETAILS DER URSPRÜNGLICHEN NACHRICHT:\n");
    body_de.push_str("--------------------------------------------------\n");
    body_en.push_str("--------------------------------------------------\n");
    body_en.push_str("ORIGINAL MESSAGE DETAILS:\n");
    body_en.push_str("--------------------------------------------------\n");
    body_de.push_str(&format!("Betreff: {}\n", original_subject));
    body_de.push_str(&format!("Datum: {}\n", date_header));
    body_de.push_str(&format!("Message-ID: {}\n\n", message_id_header));
    body_en.push_str(&format!("Subject: {}\n", original_subject));
    body_en.push_str(&format!("Date: {}\n", date_header));
    body_en.push_str(&format!("Message-ID: {}\n\n", message_id_header));

    body_de.push_str("--------------------------------------------------\n");
    body_de.push_str("HINWEIS:\n");
    body_en.push_str("--------------------------------------------------\n");
    body_en.push_str("NOTE:\n");
    body_de.push_str(
        "Dies ist eine automatisch generierte Benachrichtigung des Kommunikationszentrums.\n",
    );
    body_en
        .push_str("This is an automatically generated notification from Kommunikationszentrum.\n");

    (subject, body_de + "\n\n" + &body_en)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_bounce_recipient() {
        assert!(is_valid_bounce_recipient("user@example.com"));
        assert!(is_valid_bounce_recipient("member+tag@solawi.org"));

        assert!(!is_valid_bounce_recipient(""));
        assert!(!is_valid_bounce_recipient("   "));
        assert!(!is_valid_bounce_recipient("<>"));
        assert!(!is_valid_bounce_recipient("unknown"));
        assert!(!is_valid_bounce_recipient("UNKNOWN"));
        assert!(!is_valid_bounce_recipient("no-at-sign"));
        assert!(!is_valid_bounce_recipient("mailer-daemon@example.com"));
        assert!(!is_valid_bounce_recipient("MAILER-DAEMON@example.com"));
        assert!(!is_valid_bounce_recipient("postmaster@example.com"));
        assert!(!is_valid_bounce_recipient("no-reply@example.com"));
        assert!(!is_valid_bounce_recipient("noreply@example.com"));
    }

    #[test]
    fn test_build_rejection_email_no_write_permission() {
        let rejected = vec![RejectedTopic {
            topic_name: "Gartenbau".to_string(),
            topic_email: "gartenbau@solawi.org".to_string(),
            reason: TopicRejectionReason::NoWritePermission,
        }];

        let (subject, body) =
            build_rejection_email("Frage zur Ernte", None, &rejected, "member@example.com");

        assert_eq!(subject, "Rejected: Frage zur Ernte");
        assert!(body.contains("Gartenbau <gartenbau@solawi.org>"));
        assert!(body.contains("keine Schreibberechtigung"));
        assert!(body.contains("read-only"));
        assert!(body.contains("Betreff: Frage zur Ernte"));
        assert!(body.contains("Subject: Frage zur Ernte"));
    }

    #[test]
    fn test_build_rejection_email_not_subscribed() {
        let rejected = vec![RejectedTopic {
            topic_name: "Verteilpunkt Nord".to_string(),
            topic_email: "vp-nord@solawi.org".to_string(),
            reason: TopicRejectionReason::NotSubscribed,
        }];

        let (subject, body) =
            build_rejection_email("Abholung morgen", None, &rejected, "member@example.com");

        assert_eq!(subject, "Rejected: Abholung morgen");
        assert!(body.contains("Verteilpunkt Nord <vp-nord@solawi.org>"));
        assert!(body.contains("nicht abonniert"));
        assert!(body.contains("not subscribed"));
    }

    #[test]
    fn test_build_rejection_email_not_registered() {
        let rejected = vec![RejectedTopic {
            topic_name: "Verteilpunkt Süd".to_string(),
            topic_email: "vp-sued@solawi.org".to_string(),
            reason: TopicRejectionReason::NotRegistered,
        }];

        let (subject, body) =
            build_rejection_email("Hallo", None, &rejected, "external@stranger.org");

        assert_eq!(subject, "Rejected: Hallo");
        assert!(body.contains("external@stranger.org"));
        assert!(body.contains("keinem aktiven Mitgliedskonto"));
        assert!(body.contains("not associated with an active member account"));
    }

    #[test]
    fn test_build_rejection_email_fallback_subject() {
        let rejected = vec![RejectedTopic {
            topic_name: "Aktuelles".to_string(),
            topic_email: "aktuelles@solawi.org".to_string(),
            reason: TopicRejectionReason::NotSubscribed,
        }];

        let (subject, _) =
            build_rejection_email("No subject", None, &rejected, "external@stranger.org");

        assert_eq!(subject, "Rejected: Mail to aktuelles@solawi.org");
    }
}
