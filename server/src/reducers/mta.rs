use crate::models::mta::mta_connection_log;
use spacetimedb::{ReducerContext, Table};

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
}
