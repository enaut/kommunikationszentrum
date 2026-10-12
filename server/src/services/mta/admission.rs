use crate::models::mta::{MessageDisposition, TopicRejectionReason};

/// Sender facts used by the DATA-stage admission policy.
///
/// Subscription flags refer to the **exact** sending address (`account_email_id`),
/// never to other addresses on the same account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenderEligibility {
    Admin,
    VerifiedActive {
        write: bool,
        subscribed_active: bool,
    },
    /// Unknown address, unverified address, or inactive account.
    NotRegistered,
}

/// Per-topic admission decision. Oversize is evaluated before authorization
/// for every sender, including admins.
pub fn decide_topic_admission(
    oversize: bool,
    eligibility: SenderEligibility,
) -> MessageDisposition {
    if oversize {
        return MessageDisposition::Rejected(TopicRejectionReason::Oversize);
    }

    match eligibility {
        SenderEligibility::Admin => MessageDisposition::Accepted,
        SenderEligibility::NotRegistered => {
            MessageDisposition::Rejected(TopicRejectionReason::NotRegistered)
        }
        SenderEligibility::VerifiedActive {
            write,
            subscribed_active,
        } => {
            if subscribed_active && write {
                MessageDisposition::Accepted
            } else if subscribed_active {
                MessageDisposition::Rejected(TopicRejectionReason::NoWritePermission)
            } else {
                MessageDisposition::Rejected(TopicRejectionReason::NotSubscribed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepted() -> MessageDisposition {
        MessageDisposition::Accepted
    }

    fn rejected(reason: TopicRejectionReason) -> MessageDisposition {
        MessageDisposition::Rejected(reason)
    }

    #[test]
    fn admission_cases() {
        let write = SenderEligibility::VerifiedActive {
            write: true,
            subscribed_active: true,
        };
        let read_only = SenderEligibility::VerifiedActive {
            write: false,
            subscribed_active: true,
        };
        let no_subscription = SenderEligibility::VerifiedActive {
            write: false,
            subscribed_active: false,
        };
        // Write exists on a different email of the same account: this address
        // has no active subscription.
        let other_email_of_account = SenderEligibility::VerifiedActive {
            write: false,
            subscribed_active: false,
        };

        let cases: &[(&str, bool, SenderEligibility, MessageDisposition)] = &[
            ("admin", false, SenderEligibility::Admin, accepted()),
            ("verified_active_write", false, write, accepted()),
            (
                "read_only",
                false,
                read_only,
                rejected(TopicRejectionReason::NoWritePermission),
            ),
            (
                "no_subscription",
                false,
                no_subscription,
                rejected(TopicRejectionReason::NotSubscribed),
            ),
            (
                "subscription_on_different_email",
                false,
                other_email_of_account,
                rejected(TopicRejectionReason::NotSubscribed),
            ),
            (
                "unverified",
                false,
                SenderEligibility::NotRegistered,
                rejected(TopicRejectionReason::NotRegistered),
            ),
            (
                "inactive_account",
                false,
                SenderEligibility::NotRegistered,
                rejected(TopicRejectionReason::NotRegistered),
            ),
            (
                "unknown_address",
                false,
                SenderEligibility::NotRegistered,
                rejected(TopicRejectionReason::NotRegistered),
            ),
            (
                "oversize",
                true,
                write,
                rejected(TopicRejectionReason::Oversize),
            ),
            (
                "oversize_admin",
                true,
                SenderEligibility::Admin,
                rejected(TopicRejectionReason::Oversize),
            ),
        ];

        for (name, oversize, eligibility, expected) in cases {
            assert_eq!(
                decide_topic_admission(*oversize, *eligibility),
                *expected,
                "{name}"
            );
        }
    }

    #[test]
    fn multi_topic_mixture() {
        let oversize = false;
        let decisions = [
            decide_topic_admission(
                oversize,
                SenderEligibility::VerifiedActive {
                    write: true,
                    subscribed_active: true,
                },
            ),
            decide_topic_admission(
                oversize,
                SenderEligibility::VerifiedActive {
                    write: false,
                    subscribed_active: true,
                },
            ),
            decide_topic_admission(oversize, SenderEligibility::NotRegistered),
        ];

        assert_eq!(
            decisions,
            [
                MessageDisposition::Accepted,
                MessageDisposition::Rejected(TopicRejectionReason::NoWritePermission),
                MessageDisposition::Rejected(TopicRejectionReason::NotRegistered),
            ]
        );
    }

    #[test]
    fn oversize_rejects_every_topic_in_a_mixture() {
        let decisions = [
            decide_topic_admission(true, SenderEligibility::Admin),
            decide_topic_admission(
                true,
                SenderEligibility::VerifiedActive {
                    write: true,
                    subscribed_active: true,
                },
            ),
        ];

        assert!(
            decisions
                .iter()
                .all(|d| { *d == MessageDisposition::Rejected(TopicRejectionReason::Oversize) })
        );
    }

    #[test]
    fn test_admission_policy_all_acceptance_criteria() {
        // 1. Admin (always accepted)
        assert_eq!(
            decide_topic_admission(false, SenderEligibility::Admin),
            MessageDisposition::Accepted,
            "admin without subscription is accepted"
        );

        // 2. Verified + active + Write subscription (accepted)
        assert_eq!(
            decide_topic_admission(
                false,
                SenderEligibility::VerifiedActive {
                    write: true,
                    subscribed_active: true,
                }
            ),
            MessageDisposition::Accepted,
            "verified active with write subscription is accepted"
        );

        // 3. Read-only (rejected with NoWritePermission)
        assert_eq!(
            decide_topic_admission(
                false,
                SenderEligibility::VerifiedActive {
                    write: false,
                    subscribed_active: true,
                }
            ),
            MessageDisposition::Rejected(TopicRejectionReason::NoWritePermission),
            "read-only subscription is rejected with NoWritePermission"
        );

        // 4. No subscription (rejected with NotSubscribed)
        assert_eq!(
            decide_topic_admission(
                false,
                SenderEligibility::VerifiedActive {
                    write: false,
                    subscribed_active: false,
                }
            ),
            MessageDisposition::Rejected(TopicRejectionReason::NotSubscribed),
            "no subscription is rejected with NotSubscribed"
        );

        // 5. Not registered (unverified, inactive, or unknown address rejected with NotRegistered)
        assert_eq!(
            decide_topic_admission(false, SenderEligibility::NotRegistered),
            MessageDisposition::Rejected(TopicRejectionReason::NotRegistered),
            "not registered address is rejected with NotRegistered"
        );

        // 6. Oversize (>2MB) (rejected with Oversize for all senders, including admins)
        assert_eq!(
            decide_topic_admission(true, SenderEligibility::Admin),
            MessageDisposition::Rejected(TopicRejectionReason::Oversize),
            "oversize admin is rejected with Oversize"
        );
        assert_eq!(
            decide_topic_admission(
                true,
                SenderEligibility::VerifiedActive {
                    write: true,
                    subscribed_active: true,
                }
            ),
            MessageDisposition::Rejected(TopicRejectionReason::Oversize),
            "oversize verified active is rejected with Oversize"
        );
        assert_eq!(
            decide_topic_admission(true, SenderEligibility::NotRegistered),
            MessageDisposition::Rejected(TopicRejectionReason::Oversize),
            "oversize not registered is rejected with Oversize"
        );

        // 7. Multi-topic mixtures
        let topic_results = [
            decide_topic_admission(
                false,
                SenderEligibility::VerifiedActive {
                    write: true,
                    subscribed_active: true,
                },
            ),
            decide_topic_admission(
                false,
                SenderEligibility::VerifiedActive {
                    write: false,
                    subscribed_active: true,
                },
            ),
            decide_topic_admission(
                false,
                SenderEligibility::VerifiedActive {
                    write: false,
                    subscribed_active: false,
                },
            ),
        ];
        assert_eq!(
            topic_results,
            [
                MessageDisposition::Accepted,
                MessageDisposition::Rejected(TopicRejectionReason::NoWritePermission),
                MessageDisposition::Rejected(TopicRejectionReason::NotSubscribed),
            ],
            "multi-topic mixture evaluates each topic independently"
        );
    }
}
