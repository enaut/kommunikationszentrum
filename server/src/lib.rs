use std::time::Duration;

use spacetimedb::{ReducerContext, ScheduleAt, Table};

use crate::models::{
    account::{AdminIdentity, admin_identities},
    config::{MailRetentionConfig, mail_retention_config},
    mta::{PurgeRejectedMailsSchedule, purge_rejected_mails_schedule},
    sender::{JmapMailDispatchSchedule, jmap_mail_dispatch_schedule},
};
use crate::services::mta::purge::DEFAULT_REJECTED_RETENTION_DAYS;

pub mod common;
pub mod config;
pub mod http_handlers;
pub mod models;
pub mod reducers;
pub mod services;
pub mod views;

#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) {
    // Called when the module is initially published
    // let module_id = ctx.database_identity();
    let sender_identity = ctx.sender();

    if ctx
        .db
        .admin_identities()
        .identity()
        .find(sender_identity)
        .is_none()
    {
        ctx.db.admin_identities().insert(AdminIdentity {
            identity: sender_identity,
        });
        log::info!("Seeded sender identity as admin: {:?}", sender_identity);
    }

    if ctx
        .db
        .purge_rejected_mails_schedule()
        .scheduled_id()
        .find(&0)
        .is_none()
    {
        ctx.db
            .purge_rejected_mails_schedule()
            .insert(PurgeRejectedMailsSchedule {
                scheduled_id: 0,
                scheduled_at: ScheduleAt::Interval(Duration::from_secs(3600).into()),
            });
        log::info!("Seeded purge_rejected_mails schedule");
    }

    if ctx
        .db
        .jmap_mail_dispatch_schedule()
        .scheduled_id()
        .find(&0)
        .is_none()
    {
        ctx.db
            .jmap_mail_dispatch_schedule()
            .insert(JmapMailDispatchSchedule {
                scheduled_id: 0,
                scheduled_at: ScheduleAt::Interval(Duration::from_secs(10).into()),
            });
        log::info!(
            "Seeded native JMAP mail dispatch schedule (disabled until JMAP_DISPATCH_ENABLED=true)"
        );
    }

    if ctx.db.mail_retention_config().id().find(&0).is_none() {
        ctx.db.mail_retention_config().insert(MailRetentionConfig {
            id: 0,
            rejected_retention_days: DEFAULT_REJECTED_RETENTION_DAYS,
            updated_at: ctx.timestamp,
        });
        log::info!("Seeded mail_retention_config with default retention");
    }
}

#[spacetimedb::reducer(client_connected)]
pub fn identity_connected(ctx: &ReducerContext) {
    // Called everytime a new client connects
    log::info!("Client connected with identity: {:?}", ctx.sender());
}

#[spacetimedb::reducer(client_disconnected)]
pub fn identity_disconnected(_ctx: &ReducerContext) {
    // Called everytime a client disconnects
}
