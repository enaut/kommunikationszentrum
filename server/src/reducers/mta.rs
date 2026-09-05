use spacetimedb::{ReducerContext, Table};
use crate::models::mta::{mta_connection_log, mta_message_log};

#[spacetimedb::reducer]
pub fn dump_mta_logs_to_server_logs(ctx: &ReducerContext) {
    log::info!("=== MTA Connection Logs ===");
    for log in ctx.db.mta_connection_log().iter() {
        log::info!(
            "Connection Log {}: {} - {} - {}",
            log.id,
            log.stage,
            log.action,
            log.details
        );
    }

    log::info!("=== MTA Message Logs ===");
    for log in ctx.db.mta_message_log().iter() {
        log::info!(
            "Message Log {}: {} - {} - Categories: {}",
            log.id,
            log.stage,
            log.action,
            log.category_count
        );
    }
}
