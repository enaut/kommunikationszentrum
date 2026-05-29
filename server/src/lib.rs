use spacetimedb::{ReducerContext, Table};

use account::{admin_identities, AdminIdentity};

mod account;
mod mailing;
mod mta;

#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) {
    // Called when the module is initially published
    let module_id = ctx.database_identity();
    let mut exists = false;
    for row in ctx.db.admin_identities().iter() {
        if row.identity == module_id {
            exists = true;
            break;
        }
    }
    if !exists {
        ctx.db.admin_identities().insert(AdminIdentity {
            identity: module_id,
        });
        log::info!("Seeded module identity as admin: {:?}", module_id);
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
