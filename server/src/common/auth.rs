use log::info;
use spacetimedb::{CtxDbRead, CtxWithSender, Identity};

use crate::models::account::admin_identities__view;

/// Check if the current user has admin permissions.
/// Works with any context that exposes a sender and read-only DB access
/// (`ReducerContext`, `ViewContext`, `TxContext`, …).
pub fn is_admin_user(ctx: &(impl CtxDbRead + CtxWithSender)) -> bool {
    is_admin_identity(ctx, ctx.sender())
}

/// True if the provided identity is the module identity or listed in admin_identities.
///
/// Generic over [`CtxDbRead`] so it can be shared by reducers, views, and
/// procedure/HTTP transactions (`ReducerContext`, `ViewContext`, `TxContext`, …).
pub fn is_admin_identity(ctx: &impl CtxDbRead, who: Identity) -> bool {
    // Same host call as `ReducerContext::database_identity()` — available outside reducers.
    let module_identity = Identity::from_byte_array(spacetimedb::sys::identity());
    if who == module_identity {
        info!("is_admin_identity: caller is module identity");
        return true;
    }
    let res = ctx
        .db_read_only()
        .admin_identities()
        .identity()
        .find(&who)
        .is_some();
    info!("is_admin_identity: caller is admin identity: {}", res);
    res
}
