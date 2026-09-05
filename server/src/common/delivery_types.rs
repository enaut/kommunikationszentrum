use spacetimedb::{Identity, SpacetimeType, TimeDuration, Timestamp};

pub const MAX_INGRESS_ATTEMPTS: u32 = 3;

const MICROS_PER_SEC: i64 = 1_000_000;
const MICROS_PER_MIN: i64 = 60 * MICROS_PER_SEC;

pub fn ingress_lease_duration() -> TimeDuration {
    TimeDuration::from_micros(10 * MICROS_PER_MIN)
}

pub fn delivery_lease_duration() -> TimeDuration {
    TimeDuration::from_micros(5 * MICROS_PER_MIN)
}

pub fn ingress_retry_backoff(attempt_count: u32) -> TimeDuration {
    match attempt_count {
        1 => TimeDuration::from_micros(30 * MICROS_PER_SEC),
        2 => TimeDuration::from_micros(2 * MICROS_PER_MIN),
        3 => TimeDuration::from_micros(10 * MICROS_PER_MIN),
        _ => TimeDuration::from_micros(30 * MICROS_PER_MIN),
    }
}

/// State values shared between the ingress and delivery pipelines.
#[derive(Clone, PartialEq, Eq, SpacetimeType)]
pub enum DeliveryStatus {
    // Ingress states
    Pending,
    Processing,
    RetryScheduled,
    Completed,
    Failed,
    // Delivery states
    Queued,
    Sending,
    Sent,
    Bounced,
}

impl DeliveryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::RetryScheduled => "retry_scheduled",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Bounced => "bounced",
        }
    }
}

impl std::fmt::Display for DeliveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Terminal state for delivery items.
#[derive(Clone, Copy, PartialEq, Eq, SpacetimeType)]
pub enum DeliveryFinalState {
    Sent,
    Failed,
    Bounced,
    Cancelled,
}

impl DeliveryFinalState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Bounced => "bounced",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for DeliveryFinalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Claim/lease state embedded in queue rows.
#[derive(Clone, SpacetimeType)]
pub struct ClaimState {
    pub status: DeliveryStatus,
    pub next_attempt_at: Timestamp,
    pub claim_owner: Option<Identity>,
    pub instance_id: Option<String>,
    pub claim_expires_at: Timestamp,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub updated_at: Timestamp,
}

impl ClaimState {
    pub fn new_pending(now: Timestamp) -> Self {
        Self {
            status: DeliveryStatus::Pending,
            next_attempt_at: now,
            claim_owner: None,
            instance_id: None,
            claim_expires_at: Timestamp::UNIX_EPOCH,
            attempt_count: 0,
            last_error: None,
            updated_at: now,
        }
    }
}
