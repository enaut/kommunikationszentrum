use opentelemetry::propagation::TextMapPropagator;
use opentelemetry::Context;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use std::collections::HashMap;

/// Derive a deterministic W3C traceparent from a Stalwart queue-id string.
///
/// Stalwart does not propagate `traceparent` headers into MTA hook requests, so
/// we synthesise one from the queue-id that is already stored on `MailMessage`.
/// Using a deterministic hash means the same queue-id always produces the same
/// trace-id, which lets you correlate Stalwart log lines (which include the
/// queue-id as plain text) with Tempo traces even though Stalwart's own spans
/// currently carry independent trace-ids.
///
/// Format: `00-<32 hex trace-id>-<16 hex span-id>-01`
pub fn traceparent_from_queue_id(queue_id: &str) -> String {
    let hash = blake3::hash(queue_id.as_bytes());
    let bytes = hash.as_bytes();

    // First 16 bytes → 32 hex chars for the trace-id
    let trace_id = hex::encode(&bytes[..16]);
    // Next 8 bytes → 16 hex chars for the root span-id
    let span_id = hex::encode(&bytes[16..24]);

    format!("00-{trace_id}-{span_id}-01")
}

/// Extract the 32-hex trace-id from a W3C traceparent string.
///
/// W3C format: `00-<trace_id_32hex>-<span_id_16hex>-<flags_2hex>`
///
/// We log this explicitly as a structured field so that:
/// - The correct Stalwart-derived trace-id appears in Loki log records
///   (rather than relying on the OTel bridge's automatic injection, which
///    reflects the _local_ span's trace-id at log time)
/// - The Loki datasource "Derived fields" regex and the dashboard table panel
///   can both resolve it into a clickable Tempo link.
pub fn trace_id_from_traceparent(traceparent: &str) -> &str {
    // "00-<32hex>-<16hex>-01"
    //      ^   ^  split by '-', second element
    traceparent
        .splitn(4, '-')
        .nth(1)
        .unwrap_or("")
}

/// Extract an OpenTelemetry [`Context`] from a W3C `traceparent` string so
/// that spans started under the returned context become children of the remote
/// trace represented by that `traceparent`.
pub fn context_from_traceparent(traceparent: &str) -> Context {
    let propagator = TraceContextPropagator::new();
    let mut carrier = HashMap::new();
    carrier.insert("traceparent".to_string(), traceparent.to_string());
    propagator.extract(&carrier)
}
