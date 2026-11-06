use log::info;
use spacetimedb::{CtxDbRead, CtxWithSender, Identity};

use crate::models::account::{account__view, admin_identities__view};
use crate::models::topic::subscriptions__view;

/// Check if the current user has admin permissions.
/// Works with any context that exposes a sender and read-only DB access
/// (`ReducerContext`, `ViewContext`, `TxContext`, …).
pub fn is_admin_user(ctx: &(impl CtxDbRead + CtxWithSender)) -> bool {
    is_admin_identity(ctx, ctx.sender())
}

/// True for the module identity, a registered account-less service identity, or an active,
/// non-deleted account identity that is registered as an admin.
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
    let account_status = db
        .account()
        .identity()
        .find(&who)
        .map(|account| (account.is_active, account.is_deleted_from_idp));
    let has_admin_entry = db.admin_identities().identity().find(&who).is_some();
    let res = is_registered_admin_authorized(has_admin_entry, account_status);
    info!("is_admin_identity: caller is admin identity: {}", res);
    res
}

fn is_registered_admin_authorized(
    has_admin_entry: bool,
    account_status: Option<(bool, bool)>,
) -> bool {
    has_admin_entry && account_status.is_none_or(|(is_active, is_deleted)| is_active && !is_deleted)
}

#[cfg(test)]
mod tests {
    use super::is_registered_admin_authorized;

    #[test]
    fn registered_service_identity_without_account_is_authorized() {
        assert!(is_registered_admin_authorized(true, None));
    }

    #[test]
    fn account_linked_admin_must_be_active_and_not_deleted() {
        assert!(is_registered_admin_authorized(true, Some((true, false))));
        assert!(!is_registered_admin_authorized(true, Some((false, false))));
        assert!(!is_registered_admin_authorized(true, Some((true, true))));
    }

    #[test]
    fn identity_without_admin_entry_is_not_authorized() {
        assert!(!is_registered_admin_authorized(false, None));
        assert!(!is_registered_admin_authorized(false, Some((true, false))));
    }
}

/// Check if the caller identity has active moderation rights on the given topic.
pub fn is_topic_moderator(ctx: &(impl CtxDbRead + CtxWithSender), topic_id: u64) -> bool {
    let db = ctx.db_read_only();
    let who = ctx.sender();
    let Some(account) = db.account().identity().find(&who) else {
        return false;
    };
    if !account.is_active || account.is_deleted_from_idp {
        return false;
    }
    db.subscriptions()
        .subscriber_account_id()
        .filter(&account.id)
        .any(|s| s.topic_id == topic_id && s.status.is_active() && s.permission.is_moderator())
}

/// Check if the caller has admin permissions or moderates the given topic.
pub fn is_admin_or_topic_moderator(ctx: &(impl CtxDbRead + CtxWithSender), topic_id: u64) -> bool {
    is_admin_user(ctx) || is_topic_moderator(ctx, topic_id)
}

/// Check if the caller has active moderation rights on at least one topic.
pub fn is_any_topic_moderator(ctx: &(impl CtxDbRead + CtxWithSender)) -> bool {
    let db = ctx.db_read_only();
    let who = ctx.sender();
    let Some(account) = db.account().identity().find(&who) else {
        return false;
    };
    if !account.is_active || account.is_deleted_from_idp {
        return false;
    }
    db.subscriptions()
        .subscriber_account_id()
        .filter(&account.id)
        .any(|s| s.status.is_active() && s.permission.is_moderator())
}
