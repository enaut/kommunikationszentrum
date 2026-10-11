use spacetimedb::ReducerContext;

use crate::{
    common::auth::is_admin_identity,
    models::{config::mail_retention_config, mta::PurgeRejectedMailsSchedule},
    services::mta::purge::{
        DEFAULT_PURGE_BATCH_LIMIT, DEFAULT_REJECTED_RETENTION_DAYS, SpacetimePurgeStore,
        calculate_purge_cutoff, purge_rejected_messages,
    },
};

#[spacetimedb::reducer]
pub fn purge_rejected_mails(
    ctx: &ReducerContext,
    _scheduled: PurgeRejectedMailsSchedule,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }

    let retention_days = ctx
        .db
        .mail_retention_config()
        .id()
        .find(&0)
        .map(|c| c.rejected_retention_days)
        .unwrap_or(DEFAULT_REJECTED_RETENTION_DAYS);

    let cutoff = calculate_purge_cutoff(ctx.timestamp, retention_days);
    let mut store = SpacetimePurgeStore::new(ctx);
    let stats = purge_rejected_messages(&mut store, cutoff, DEFAULT_PURGE_BATCH_LIMIT);

    log::info!(
        "purge_rejected_mails executed: retention_days={}, cutoff={:?}, purged_received={}, purged_mail_messages={}",
        retention_days,
        cutoff,
        stats.purged_received_messages,
        stats.purged_mail_messages
    );

    Ok(())
}
