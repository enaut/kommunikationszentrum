use spacetimedb::{ReducerContext, Table};

use account::{admin_identities, AdminIdentity};

mod account;
mod http_handlers;
mod mailing;
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
