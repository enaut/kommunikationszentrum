use crate::module_bindings::SubscriptionStatus;

pub mod category;
pub mod management;
pub mod members;
pub mod messages;
pub mod subscriptions;

/// Whether a subscription with this status should currently receive mail.
/// Mirrors `SubscriptionStatus::is_active` on the server.
pub fn is_active_subscription(status: &SubscriptionStatus) -> bool {
    matches!(
        status,
        SubscriptionStatus::AutomaticallySubscribed
            | SubscriptionStatus::ManuallySubscribed
            | SubscriptionStatus::RequiredSubscribed
    )
}
