use crate::module_bindings::{Account, AccountEmail, Subscription, SubscriptionStatus};

/// Whether a subscription with this status should currently receive mail.
/// Mirrors `SubscriptionStatus::is_active` on the server (`server::models::topic::SubscriptionStatus`).
///
/// Exhaustive matching without wildcards is intentionally used here so that
/// any newly added `SubscriptionStatus` variant causes a compile-time error,
/// preventing drift between the sender and server definitions.
pub fn is_active_subscription(status: &SubscriptionStatus) -> bool {
    match status {
        SubscriptionStatus::AutomaticallySubscribed
        | SubscriptionStatus::ManuallySubscribed
        | SubscriptionStatus::RequiredSubscribed => true,
        SubscriptionStatus::AutomaticallyUnsubscribed
        | SubscriptionStatus::ManuallyUnsubscribed
        | SubscriptionStatus::LinkUnsubscribed => false,
    }
}

/// Pure filter evaluating whether a subscription candidate is eligible to receive mail.
///
/// A recipient is eligible only if:
/// 1. The subscription is active (`SubscriptionStatus::is_active`).
/// 2. The linked `account_email` exists, belongs to the subscriber account, and is verified.
/// 3. The subscriber `Account` exists and is active (`account.is_active == true`).
///
/// Returns the verified recipient email string if eligible, or `None` if skipped.
pub fn filter_eligible_recipient(
    subscription: &Subscription,
    account_email: Option<&AccountEmail>,
    account: Option<&Account>,
) -> Option<String> {
    if !is_active_subscription(&subscription.status) {
        return None;
    }

    let email = account_email?;
    if email.account_id != subscription.subscriber_account_id || !email.is_verified {
        return None;
    }

    let account = account?;
    if !account.is_active {
        return None;
    }

    Some(email.email.clone())
}

/// Pure resolution function that extracts all eligible recipients for a given topic.
///
/// Given subscriptions and lookups for `account_email` and `account`, resolves
/// each subscription for `topic_id` to its eligible verified recipient email and subscription pair.
pub fn resolve_topic_subscribers<S, FEmail, FAccount>(
    topic_id: u64,
    subscriptions: S,
    find_email: FEmail,
    find_account: FAccount,
) -> Vec<(String, Subscription)>
where
    S: IntoIterator<Item = Subscription>,
    FEmail: Fn(u64) -> Option<AccountEmail>,
    FAccount: Fn(u64) -> Option<Account>,
{
    subscriptions
        .into_iter()
        .filter(|sub| sub.topic_id == topic_id)
        .filter_map(|sub| {
            let email = find_email(sub.account_email_id);
            let account = find_account(sub.subscriber_account_id);
            let recipient_email =
                filter_eligible_recipient(&sub, email.as_ref(), account.as_ref())?;
            Some((recipient_email, sub))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module_bindings::{EmailSource, SubscriptionPermission};
    use spacetimedb_sdk::{Identity, Timestamp};

    fn dummy_account(id: u64, is_active: bool) -> Account {
        Account {
            id,
            external_id: format!("ext-{}", id),
            identity: Identity::from_byte_array([0u8; 32]),
            name: format!("User {}", id),
            primary_email_id: id * 10,
            is_active,
            last_synced: Timestamp::UNIX_EPOCH,
        }
    }

    fn dummy_email(id: u64, account_id: u64, email: &str, is_verified: bool) -> AccountEmail {
        AccountEmail {
            id,
            account_id,
            email: email.to_string(),
            source: EmailSource::ExternalSync,
            is_verified,
            added_at: Timestamp::UNIX_EPOCH,
        }
    }

    fn dummy_subscription(
        id: u64,
        topic_id: u64,
        subscriber_account_id: u64,
        account_email_id: u64,
        status: SubscriptionStatus,
    ) -> Subscription {
        Subscription {
            id,
            subscriber_account_id,
            account_email_id,
            topic_id,
            subscribed_at: Timestamp::UNIX_EPOCH,
            status,
            permission: SubscriptionPermission::Write,
        }
    }

    #[test]
    fn test_is_active_subscription_covers_all_variants() {
        assert!(is_active_subscription(&SubscriptionStatus::AutomaticallySubscribed));
        assert!(is_active_subscription(&SubscriptionStatus::ManuallySubscribed));
        assert!(is_active_subscription(&SubscriptionStatus::RequiredSubscribed));

        assert!(!is_active_subscription(&SubscriptionStatus::AutomaticallyUnsubscribed));
        assert!(!is_active_subscription(&SubscriptionStatus::ManuallyUnsubscribed));
        assert!(!is_active_subscription(&SubscriptionStatus::LinkUnsubscribed));
    }

    #[test]
    fn test_active_recipient_on_active_account_is_eligible() {
        let sub = dummy_subscription(1, 100, 10, 20, SubscriptionStatus::ManuallySubscribed);
        let email = dummy_email(20, 10, "alice@example.com", true);
        let account = dummy_account(10, true);

        let result = filter_eligible_recipient(&sub, Some(&email), Some(&account));
        assert_eq!(result, Some("alice@example.com".to_string()));
    }

    #[test]
    fn test_recipient_on_deactivated_account_is_not_eligible() {
        let sub = dummy_subscription(1, 100, 10, 20, SubscriptionStatus::ManuallySubscribed);
        let email = dummy_email(20, 10, "alice@example.com", true);
        let deactivated_account = dummy_account(10, false);

        let result = filter_eligible_recipient(&sub, Some(&email), Some(&deactivated_account));
        assert_eq!(result, None);
    }

    #[test]
    fn test_recipient_with_missing_account_is_not_eligible() {
        let sub = dummy_subscription(1, 100, 10, 20, SubscriptionStatus::ManuallySubscribed);
        let email = dummy_email(20, 10, "alice@example.com", true);

        let result = filter_eligible_recipient(&sub, Some(&email), None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_recipient_with_unverified_email_is_skipped() {
        let sub = dummy_subscription(1, 100, 10, 20, SubscriptionStatus::ManuallySubscribed);
        let email = dummy_email(20, 10, "alice@example.com", false);
        let account = dummy_account(10, true);

        let result = filter_eligible_recipient(&sub, Some(&email), Some(&account));
        assert_eq!(result, None);
    }

    #[test]
    fn test_recipient_with_email_belonging_to_different_account_is_skipped() {
        let sub = dummy_subscription(1, 100, 10, 20, SubscriptionStatus::ManuallySubscribed);
        let wrong_account_email = dummy_email(20, 999, "alice@example.com", true);
        let account = dummy_account(10, true);

        let result = filter_eligible_recipient(&sub, Some(&wrong_account_email), Some(&account));
        assert_eq!(result, None);
    }

    #[test]
    fn test_recipient_with_missing_email_is_skipped() {
        let sub = dummy_subscription(1, 100, 10, 20, SubscriptionStatus::ManuallySubscribed);
        let account = dummy_account(10, true);

        let result = filter_eligible_recipient(&sub, None, Some(&account));
        assert_eq!(result, None);
    }

    #[test]
    fn test_recipient_with_inactive_subscription_is_skipped() {
        let email = dummy_email(20, 10, "alice@example.com", true);
        let account = dummy_account(10, true);

        for inactive_status in [
            SubscriptionStatus::AutomaticallyUnsubscribed,
            SubscriptionStatus::ManuallyUnsubscribed,
            SubscriptionStatus::LinkUnsubscribed,
        ] {
            let sub = dummy_subscription(1, 100, 10, 20, inactive_status);
            let result = filter_eligible_recipient(&sub, Some(&email), Some(&account));
            assert_eq!(result, None, "Status {:?} should not be eligible", inactive_status);
        }
    }

    #[test]
    fn test_resolve_topic_subscribers_end_to_end() {
        let topic_id = 42;
        let other_topic_id = 99;

        // Sub 1: Active account, verified email -> should be included
        let sub1 = dummy_subscription(1, topic_id, 10, 100, SubscriptionStatus::AutomaticallySubscribed);
        // Sub 2: Deactivated account, verified email -> should be excluded
        let sub2 = dummy_subscription(2, topic_id, 20, 200, SubscriptionStatus::ManuallySubscribed);
        // Sub 3: Active account, unverified email -> should be excluded
        let sub3 = dummy_subscription(3, topic_id, 30, 300, SubscriptionStatus::RequiredSubscribed);
        // Sub 4: Active account, verified email, but wrong topic -> should be excluded
        let sub4 = dummy_subscription(4, other_topic_id, 40, 400, SubscriptionStatus::ManuallySubscribed);

        let emails = vec![
            dummy_email(100, 10, "active@example.com", true),
            dummy_email(200, 20, "deactivated@example.com", true),
            dummy_email(300, 30, "unverified@example.com", false),
            dummy_email(400, 40, "other_topic@example.com", true),
        ];

        let accounts = vec![
            dummy_account(10, true),
            dummy_account(20, false),
            dummy_account(30, true),
            dummy_account(40, true),
        ];

        let subs = vec![sub1.clone(), sub2, sub3, sub4];

        let resolved = resolve_topic_subscribers(
            topic_id,
            subs,
            |email_id| emails.iter().find(|e| e.id == email_id).cloned(),
            |account_id| accounts.iter().find(|a| a.id == account_id).cloned(),
        );

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, "active@example.com");
        assert_eq!(resolved[0].1.id, sub1.id);
    }
}
