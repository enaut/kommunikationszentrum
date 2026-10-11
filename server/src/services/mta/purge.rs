use spacetimedb::{ReducerContext, TimeDuration, Timestamp};

use crate::models::{
    delivery::{mail_delivery_message, mail_ingress},
    mail_message::mail_message,
    mta::{MessageDisposition, ReceivedMessage, received_message},
};

pub const DEFAULT_REJECTED_RETENTION_DAYS: u32 = 30;
pub const DEFAULT_PURGE_BATCH_LIMIT: usize = 100;
pub const MICROS_PER_SEC: i64 = 1_000_000;
pub const MICROS_PER_DAY: i64 = 86_400 * MICROS_PER_SEC;

/// Calculate the timestamp cutoff for message retention.
/// Messages received at or before this cutoff are considered older than `retention_days`.
pub fn calculate_purge_cutoff(now: Timestamp, retention_days: u32) -> Timestamp {
    if retention_days == 0 {
        return now;
    }
    let retention_micros = (retention_days as i64).saturating_mul(MICROS_PER_DAY);
    let retention_duration = TimeDuration::from_micros(retention_micros);

    if now > Timestamp::UNIX_EPOCH + retention_duration {
        now - retention_duration
    } else {
        Timestamp::UNIX_EPOCH
    }
}

/// Input facts for the "is purgeable" predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurgePredicateInput {
    pub disposition: MessageDisposition,
    pub received_at: Timestamp,
    pub cutoff: Timestamp,
    pub other_received_refs: usize,
    pub ingress_refs: usize,
    pub delivery_refs: usize,
}

/// Decisions produced by the "is purgeable" predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurgeDecision {
    /// True if the `ReceivedMessage` row should be purged.
    pub purge_received_message: bool,
    /// True if the underlying canonical `MailMessage` row should be deleted.
    pub purge_mail_message: bool,
}

impl PurgeDecision {
    pub fn can_purge_received(&self) -> bool {
        self.purge_received_message
    }

    pub fn can_purge_mail_message(&self) -> bool {
        self.purge_mail_message
    }
}

/// Pure "is purgeable" predicate.
///
/// Rules:
/// - A `ReceivedMessage` is purgeable if and only if its disposition is `Rejected`
///   and its `received_at` timestamp is on or before `cutoff`.
/// - Its `MailMessage` is deleted if and only if the `ReceivedMessage` is purged
///   AND no other `ReceivedMessage`, `MailIngress`, or `MailDeliveryMessage` still references it.
pub fn is_purgeable(input: PurgePredicateInput) -> PurgeDecision {
    let purge_received = matches!(input.disposition, MessageDisposition::Rejected(_))
        && input.received_at <= input.cutoff;

    let purge_mail = purge_received
        && input.other_received_refs == 0
        && input.ingress_refs == 0
        && input.delivery_refs == 0;

    PurgeDecision {
        purge_received_message: purge_received,
        purge_mail_message: purge_mail,
    }
}

/// Pure helper checking if a `ReceivedMessage` is purgeable.
pub fn is_received_message_purgeable(
    disposition: &MessageDisposition,
    received_at: Timestamp,
    cutoff: Timestamp,
) -> bool {
    matches!(disposition, MessageDisposition::Rejected(_)) && received_at <= cutoff
}

/// Pure helper checking if a `MailMessage` is purgeable (i.e. has no active references).
pub fn is_mail_message_purgeable(
    other_received_refs: usize,
    ingress_refs: usize,
    delivery_refs: usize,
) -> bool {
    other_received_refs == 0 && ingress_refs == 0 && delivery_refs == 0
}

/// Storage operations needed by the purge engine.
pub trait PurgeStore {
    fn candidate_rejected_messages(&self, cutoff: Timestamp, limit: usize) -> Vec<ReceivedMessage>;
    fn delete_received_message(&mut self, id: u64) -> bool;
    fn has_other_received_message_refs(&self, mail_message_id: u64) -> bool;
    fn has_ingress_refs(&self, mail_message_id: u64) -> bool;
    fn has_delivery_refs(&self, mail_message_id: u64) -> bool;
    fn delete_mail_message(&mut self, id: u64) -> bool;
}

/// Execution stats from a purge run.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PurgeStats {
    pub purged_received_messages: usize,
    pub purged_mail_messages: usize,
}

/// Execute a batch of purges against any implementation of [`PurgeStore`].
///
/// Bounded by `batch_limit` to prevent exceeding reducer execution budgets.
pub fn purge_rejected_messages<S: PurgeStore>(
    store: &mut S,
    cutoff: Timestamp,
    batch_limit: usize,
) -> PurgeStats {
    let candidates = store.candidate_rejected_messages(cutoff, batch_limit);
    let mut stats = PurgeStats::default();

    for rm in candidates {
        if !store.delete_received_message(rm.id) {
            continue;
        }
        stats.purged_received_messages += 1;

        let has_other_received = store.has_other_received_message_refs(rm.mail_message_id);
        let has_ingress = store.has_ingress_refs(rm.mail_message_id);
        let has_delivery = store.has_delivery_refs(rm.mail_message_id);

        if !has_other_received && !has_ingress && !has_delivery {
            if store.delete_mail_message(rm.mail_message_id) {
                stats.purged_mail_messages += 1;
            }
        }
    }

    stats
}

/// Real SpacetimeDB table adapter implementing [`PurgeStore`].
pub struct SpacetimePurgeStore<'a> {
    ctx: &'a ReducerContext,
}

impl<'a> SpacetimePurgeStore<'a> {
    pub fn new(ctx: &'a ReducerContext) -> Self {
        Self { ctx }
    }
}

impl<'a> PurgeStore for SpacetimePurgeStore<'a> {
    fn candidate_rejected_messages(&self, cutoff: Timestamp, limit: usize) -> Vec<ReceivedMessage> {
        self.ctx
            .db
            .received_message()
            .received_at()
            .filter(..=cutoff)
            .filter(|rm| matches!(rm.disposition, MessageDisposition::Rejected(_)))
            .take(limit)
            .collect()
    }

    fn delete_received_message(&mut self, id: u64) -> bool {
        self.ctx.db.received_message().id().delete(&id)
    }

    fn has_other_received_message_refs(&self, mail_message_id: u64) -> bool {
        self.ctx
            .db
            .received_message()
            .mail_message_id()
            .filter(&mail_message_id)
            .next()
            .is_some()
    }

    fn has_ingress_refs(&self, mail_message_id: u64) -> bool {
        self.ctx
            .db
            .mail_ingress()
            .mail_message_id()
            .filter(&mail_message_id)
            .next()
            .is_some()
    }

    fn has_delivery_refs(&self, mail_message_id: u64) -> bool {
        self.ctx
            .db
            .mail_delivery_message()
            .mail_message_id()
            .filter(&mail_message_id)
            .next()
            .is_some()
    }

    fn delete_mail_message(&mut self, id: u64) -> bool {
        self.ctx.db.mail_message().id().delete(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::mta::TopicRejectionReason;
    use std::collections::{HashMap, HashSet};

    fn make_timestamp_days_ago(now: Timestamp, days: u32) -> Timestamp {
        let micros = (days as i64) * MICROS_PER_DAY;
        now - TimeDuration::from_micros(micros)
    }

    #[test]
    fn test_is_purgeable_predicate_all_acceptance_criteria() {
        let now = Timestamp::UNIX_EPOCH + TimeDuration::from_micros(100 * MICROS_PER_DAY);
        let cutoff = calculate_purge_cutoff(now, 30); // 70 days after UNIX_EPOCH
        let old_time = make_timestamp_days_ago(now, 40); // 60 days after UNIX_EPOCH <= cutoff
        let recent_time = make_timestamp_days_ago(now, 10); // 90 days after UNIX_EPOCH > cutoff

        let rejected_disp = MessageDisposition::Rejected(TopicRejectionReason::NotRegistered);
        let accepted_disp = MessageDisposition::Accepted;

        // 1. rejected + old -> yes (both received_message and mail_message purged)
        let case1 = is_purgeable(PurgePredicateInput {
            disposition: rejected_disp,
            received_at: old_time,
            cutoff,
            other_received_refs: 0,
            ingress_refs: 0,
            delivery_refs: 0,
        });
        assert!(
            case1.can_purge_received(),
            "rejected+old must purge received_message"
        );
        assert!(
            case1.can_purge_mail_message(),
            "rejected+old without other refs must purge mail_message"
        );
        assert!(is_received_message_purgeable(
            &rejected_disp,
            old_time,
            cutoff
        ));
        assert!(is_mail_message_purgeable(0, 0, 0));

        // 2. rejected + recent -> no
        let case2 = is_purgeable(PurgePredicateInput {
            disposition: rejected_disp,
            received_at: recent_time,
            cutoff,
            other_received_refs: 0,
            ingress_refs: 0,
            delivery_refs: 0,
        });
        assert!(
            !case2.can_purge_received(),
            "rejected+recent must NOT purge received_message"
        );
        assert!(
            !case2.can_purge_mail_message(),
            "rejected+recent must NOT purge mail_message"
        );
        assert!(!is_received_message_purgeable(
            &rejected_disp,
            recent_time,
            cutoff
        ));

        // 3. accepted + old -> no
        let case3 = is_purgeable(PurgePredicateInput {
            disposition: accepted_disp,
            received_at: old_time,
            cutoff,
            other_received_refs: 0,
            ingress_refs: 0,
            delivery_refs: 0,
        });
        assert!(
            !case3.can_purge_received(),
            "accepted+old must NOT purge received_message"
        );
        assert!(
            !case3.can_purge_mail_message(),
            "accepted+old must NOT purge mail_message"
        );
        assert!(!is_received_message_purgeable(
            &accepted_disp,
            old_time,
            cutoff
        ));

        // 4. shared mail_message still referenced -> not deleted
        // Case 4a: still referenced by another received_message
        let case4a = is_purgeable(PurgePredicateInput {
            disposition: rejected_disp,
            received_at: old_time,
            cutoff,
            other_received_refs: 1,
            ingress_refs: 0,
            delivery_refs: 0,
        });
        assert!(
            case4a.can_purge_received(),
            "rejected received_message itself is purged"
        );
        assert!(
            !case4a.can_purge_mail_message(),
            "shared mail_message with other received refs must NOT be deleted"
        );
        assert!(!is_mail_message_purgeable(1, 0, 0));

        // Case 4b: referenced by mail_ingress
        let case4b = is_purgeable(PurgePredicateInput {
            disposition: rejected_disp,
            received_at: old_time,
            cutoff,
            other_received_refs: 0,
            ingress_refs: 1,
            delivery_refs: 0,
        });
        assert!(case4b.can_purge_received());
        assert!(
            !case4b.can_purge_mail_message(),
            "mail_message referenced by mail_ingress must NOT be deleted"
        );
        assert!(!is_mail_message_purgeable(0, 1, 0));

        // Case 4c: referenced by mail_delivery_message
        let case4c = is_purgeable(PurgePredicateInput {
            disposition: rejected_disp,
            received_at: old_time,
            cutoff,
            other_received_refs: 0,
            ingress_refs: 0,
            delivery_refs: 1,
        });
        assert!(case4c.can_purge_received());
        assert!(
            !case4c.can_purge_mail_message(),
            "mail_message referenced by delivery must NOT be deleted"
        );
        assert!(!is_mail_message_purgeable(0, 0, 1));
    }

    #[test]
    fn test_changing_retention_setting_changes_cutoff() {
        let now = Timestamp::UNIX_EPOCH + TimeDuration::from_micros(100 * MICROS_PER_DAY);

        let cutoff_30 = calculate_purge_cutoff(now, 30);
        let cutoff_10 = calculate_purge_cutoff(now, 10);
        let cutoff_60 = calculate_purge_cutoff(now, 60);
        let cutoff_0 = calculate_purge_cutoff(now, 0);

        assert_eq!(
            cutoff_30,
            now - TimeDuration::from_micros(30 * MICROS_PER_DAY)
        );
        assert_eq!(
            cutoff_10,
            now - TimeDuration::from_micros(10 * MICROS_PER_DAY)
        );
        assert_eq!(
            cutoff_60,
            now - TimeDuration::from_micros(60 * MICROS_PER_DAY)
        );
        assert_eq!(cutoff_0, now);

        // A message received 20 days ago:
        let msg_time = make_timestamp_days_ago(now, 20);
        let rejected = MessageDisposition::Rejected(TopicRejectionReason::NotSubscribed);

        // Under 30 days retention: 20 days is NOT purgeable (cutoff = 30 days ago, msg_time > cutoff)
        assert!(!is_received_message_purgeable(
            &rejected, msg_time, cutoff_30
        ));

        // Under 10 days retention: 20 days IS purgeable (cutoff = 10 days ago, msg_time <= cutoff)
        assert!(is_received_message_purgeable(
            &rejected, msg_time, cutoff_10
        ));
    }

    /// In-memory implementation of PurgeStore for unit testing
    struct MockPurgeStore {
        received_messages: HashMap<u64, ReceivedMessage>,
        mail_messages: HashSet<u64>,
        ingress_mail_messages: HashSet<u64>,
        delivery_mail_messages: HashSet<u64>,
    }

    impl MockPurgeStore {
        fn new() -> Self {
            Self {
                received_messages: HashMap::new(),
                mail_messages: HashSet::new(),
                ingress_mail_messages: HashSet::new(),
                delivery_mail_messages: HashSet::new(),
            }
        }

        fn add_received_message(
            &mut self,
            id: u64,
            mail_message_id: u64,
            disposition: MessageDisposition,
            received_at: Timestamp,
        ) {
            self.mail_messages.insert(mail_message_id);
            self.received_messages.insert(
                id,
                ReceivedMessage {
                    id,
                    mail_message_id,
                    topic_id: 1,
                    topic_email: "topic@example.com".to_string(),
                    received_at,
                    disposition,
                },
            );
        }
    }

    impl PurgeStore for MockPurgeStore {
        fn candidate_rejected_messages(
            &self,
            cutoff: Timestamp,
            limit: usize,
        ) -> Vec<ReceivedMessage> {
            let mut list: Vec<ReceivedMessage> = self
                .received_messages
                .values()
                .filter(|rm| {
                    rm.received_at <= cutoff
                        && matches!(rm.disposition, MessageDisposition::Rejected(_))
                })
                .cloned()
                .collect();
            list.sort_by_key(|rm| rm.received_at);
            list.into_iter().take(limit).collect()
        }

        fn delete_received_message(&mut self, id: u64) -> bool {
            self.received_messages.remove(&id).is_some()
        }

        fn has_other_received_message_refs(&self, mail_message_id: u64) -> bool {
            self.received_messages
                .values()
                .any(|rm| rm.mail_message_id == mail_message_id)
        }

        fn has_ingress_refs(&self, mail_message_id: u64) -> bool {
            self.ingress_mail_messages.contains(&mail_message_id)
        }

        fn has_delivery_refs(&self, mail_message_id: u64) -> bool {
            self.delivery_mail_messages.contains(&mail_message_id)
        }

        fn delete_mail_message(&mut self, id: u64) -> bool {
            self.mail_messages.remove(&id)
        }
    }

    #[test]
    fn test_purge_run_deletes_rejected_and_orphaned_mail_messages_leaves_accepted_and_delivery_intact()
     {
        let now = Timestamp::UNIX_EPOCH + TimeDuration::from_micros(100 * MICROS_PER_DAY);
        let cutoff = calculate_purge_cutoff(now, 30);
        let old_time = make_timestamp_days_ago(now, 40);
        let recent_time = make_timestamp_days_ago(now, 10);

        let mut store = MockPurgeStore::new();

        // 1. Rejected + old: should be purged, MailMessage purged
        store.add_received_message(
            1,
            101,
            MessageDisposition::Rejected(TopicRejectionReason::NotRegistered),
            old_time,
        );

        // 2. Rejected + recent: untouched, MailMessage untouched
        store.add_received_message(
            2,
            102,
            MessageDisposition::Rejected(TopicRejectionReason::NotSubscribed),
            recent_time,
        );

        // 3. Accepted + old: untouched, MailMessage untouched
        store.add_received_message(3, 103, MessageDisposition::Accepted, old_time);

        // 4. Rejected + old sharing MailMessage 103 with Accepted message 3:
        // ReceivedMessage 4 purged, but MailMessage 103 MUST NOT be deleted!
        store.add_received_message(
            4,
            103,
            MessageDisposition::Rejected(TopicRejectionReason::NoWritePermission),
            old_time,
        );

        // 5. Rejected + old with MailIngress reference:
        // ReceivedMessage 5 purged, but MailMessage 105 MUST NOT be deleted!
        store.add_received_message(
            5,
            105,
            MessageDisposition::Rejected(TopicRejectionReason::Oversize),
            old_time,
        );
        store.ingress_mail_messages.insert(105);

        // 6. Rejected + old with MailDeliveryMessage reference:
        // ReceivedMessage 6 purged, but MailMessage 106 MUST NOT be deleted!
        store.add_received_message(
            6,
            106,
            MessageDisposition::Rejected(TopicRejectionReason::NotRegistered),
            old_time,
        );
        store.delivery_mail_messages.insert(106);

        // Run purge
        let stats = purge_rejected_messages(&mut store, cutoff, 100);

        // We expect ReceivedMessages 1, 4, 5, 6 to be purged (4 rows)
        assert_eq!(stats.purged_received_messages, 4);
        // Only MailMessage 101 should be deleted (1 row), because:
        // - 102 was recent (not purged)
        // - 103 is still referenced by accepted message 3
        // - 105 has ingress ref
        // - 106 has delivery ref
        assert_eq!(stats.purged_mail_messages, 1);

        // Verify remaining received_messages
        assert!(!store.received_messages.contains_key(&1));
        assert!(store.received_messages.contains_key(&2));
        assert!(store.received_messages.contains_key(&3));
        assert!(!store.received_messages.contains_key(&4));
        assert!(!store.received_messages.contains_key(&5));
        assert!(!store.received_messages.contains_key(&6));

        // Verify remaining mail_messages
        assert!(
            !store.mail_messages.contains(&101),
            "Orphaned MailMessage 101 must be deleted"
        );
        assert!(
            store.mail_messages.contains(&102),
            "Recent MailMessage 102 must be kept"
        );
        assert!(
            store.mail_messages.contains(&103),
            "Shared MailMessage 103 must be kept"
        );
        assert!(
            store.mail_messages.contains(&105),
            "Ingress-referenced MailMessage 105 must be kept"
        );
        assert!(
            store.mail_messages.contains(&106),
            "Delivery-referenced MailMessage 106 must be kept"
        );
    }

    #[test]
    fn test_batch_limit_is_respected() {
        let now = Timestamp::UNIX_EPOCH + TimeDuration::from_micros(100 * MICROS_PER_DAY);
        let cutoff = calculate_purge_cutoff(now, 30);
        let old_time = make_timestamp_days_ago(now, 50);

        let mut store = MockPurgeStore::new();

        // Add 25 old rejected messages
        for i in 1..=25 {
            store.add_received_message(
                i,
                1000 + i,
                MessageDisposition::Rejected(TopicRejectionReason::NotRegistered),
                old_time,
            );
        }

        // Run 1: limit 10
        let stats1 = purge_rejected_messages(&mut store, cutoff, 10);
        assert_eq!(stats1.purged_received_messages, 10);
        assert_eq!(stats1.purged_mail_messages, 10);
        assert_eq!(store.received_messages.len(), 15);

        // Run 2: limit 10
        let stats2 = purge_rejected_messages(&mut store, cutoff, 10);
        assert_eq!(stats2.purged_received_messages, 10);
        assert_eq!(stats2.purged_mail_messages, 10);
        assert_eq!(store.received_messages.len(), 5);

        // Run 3: limit 10 (only 5 remain)
        let stats3 = purge_rejected_messages(&mut store, cutoff, 10);
        assert_eq!(stats3.purged_received_messages, 5);
        assert_eq!(stats3.purged_mail_messages, 5);
        assert_eq!(store.received_messages.len(), 0);
        assert_eq!(store.mail_messages.len(), 0);
    }

    #[test]
    fn test_shared_rejected_messages_same_batch() {
        let now = Timestamp::UNIX_EPOCH + TimeDuration::from_micros(100 * MICROS_PER_DAY);
        let cutoff = calculate_purge_cutoff(now, 30);
        let old_time = make_timestamp_days_ago(now, 50);

        let mut store = MockPurgeStore::new();

        // Two rejected old messages referencing the same MailMessage 500
        store.add_received_message(
            1,
            500,
            MessageDisposition::Rejected(TopicRejectionReason::NotSubscribed),
            old_time,
        );
        store.add_received_message(
            2,
            500,
            MessageDisposition::Rejected(TopicRejectionReason::NoWritePermission),
            old_time,
        );

        let stats = purge_rejected_messages(&mut store, cutoff, 10);
        assert_eq!(stats.purged_received_messages, 2);
        assert_eq!(stats.purged_mail_messages, 1);
        assert!(!store.mail_messages.contains(&500));
        assert!(store.received_messages.is_empty());
    }
}
