// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit ).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct AddTestAccountsArgs {}

impl From<AddTestAccountsArgs> for super::Reducer {
    fn from(args: AddTestAccountsArgs) -> Self {
        Self::AddTestAccounts
    }
}

impl __sdk::InModule for AddTestAccountsArgs {
    type Module = super::RemoteModule;
}

pub struct AddTestAccountsCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `add_test_accounts`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait add_test_accounts {
    /// Request that the remote module invoke the reducer `add_test_accounts` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_add_test_accounts`] callbacks.
    fn add_test_accounts(&self) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `add_test_accounts`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`AddTestAccountsCallbackId`] can be passed to [`Self::remove_on_add_test_accounts`]
    /// to cancel the callback.
    fn on_add_test_accounts(
        &self,
        callback: impl FnMut(&super::ReducerEventContext) + Send + 'static,
    ) -> AddTestAccountsCallbackId;
    /// Cancel a callback previously registered by [`Self::on_add_test_accounts`],
    /// causing it not to run in the future.
    fn remove_on_add_test_accounts(&self, callback: AddTestAccountsCallbackId);
}

impl add_test_accounts for super::RemoteReducers {
    fn add_test_accounts(&self) -> __sdk::Result<()> {
        self.imp
            .call_reducer("add_test_accounts", AddTestAccountsArgs {})
    }
    fn on_add_test_accounts(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext) + Send + 'static,
    ) -> AddTestAccountsCallbackId {
        AddTestAccountsCallbackId(self.imp.on_reducer(
            "add_test_accounts",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::AddTestAccounts {},
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx)
            }),
        ))
    }
    fn remove_on_add_test_accounts(&self, callback: AddTestAccountsCallbackId) {
        self.imp.remove_on_reducer("add_test_accounts", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `add_test_accounts`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_add_test_accounts {
    /// Set the call-reducer flags for the reducer `add_test_accounts` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn add_test_accounts(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_add_test_accounts for super::SetReducerFlags {
    fn add_test_accounts(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("add_test_accounts", flags);
    }
}
