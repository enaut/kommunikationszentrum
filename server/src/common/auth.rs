use log::info;
use spacetimedb::{CtxDbRead, CtxWithSender, Identity};

use crate::models::account::{account__view, admin_identities__view};

/// Check if the current user has admin permissions.
/// Works with any context that exposes a sender and read-only DB access
/// (`ReducerContext`, `ViewContext`, `TxContext`, …).
pub fn is_admin_user(ctx: &(impl CtxDbRead + CtxWithSender)) -> bool {
    is_admin_identity(ctx, ctx.sender())
}

/// True if the provided identity is the module identity or an active, non-deleted admin.
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
    let db = ctx.db_read_only();
    let is_active_user = db
        .account()
        .identity()
        .find(&who)
        .is_some_and(|account| account.is_active && !account.is_deleted_from_idp);
    let res = is_active_user && db.admin_identities().identity().find(&who).is_some();
    info!("is_admin_identity: caller is admin identity: {}", res);
    res
}
