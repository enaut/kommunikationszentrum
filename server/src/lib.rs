use std::time::Duration;

use spacetimedb::{ReducerContext, ScheduleAt, Table};

use account::{admin_identities, AdminIdentity};
use delivery::expire_stale_delivery_claims_schedule;
use delivery::requeue_temporary_failed_mails_schedule;
use delivery::ExpireStaleDeliveryClaimsSchedule;
use delivery::RequeueTemporaryFailedMailsSchedule;

mod account;
mod categories;
mod delivery;
mod domain;
mod http_handlers;
mod mail_message;
mod mta;

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

    // Seed the schedule for expire_stale_delivery_claims if it doesn't exist
    if ctx
        .db
        .expire_stale_delivery_claims_schedule()
        .scheduled_id()
        .find(&0)
        .is_none()
    {
        ctx.db
            .expire_stale_delivery_claims_schedule()
            .insert(ExpireStaleDeliveryClaimsSchedule {
                scheduled_id: 0,
                scheduled_at: ScheduleAt::Interval(Duration::from_secs(60).into()),
            });
        log::info!("Seeded expire_stale_delivery_claims schedule");
    }

    if ctx
        .db
        .requeue_temporary_failed_mails_schedule()
        .scheduled_id()
        .find(&0)
        .is_none()
    {
        ctx.db.requeue_temporary_failed_mails_schedule().insert(
            RequeueTemporaryFailedMailsSchedule {
                scheduled_id: 0,
                scheduled_at: ScheduleAt::Interval(Duration::from_secs(10).into()),
            },
        );
        log::info!("Seeded requeue_temporary_failed_mails schedule");
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
