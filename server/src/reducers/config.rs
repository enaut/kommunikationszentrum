use crate::common::auth::is_admin_identity;
use crate::models::config::*;
use spacetimedb::{ReducerContext, Table};

#[spacetimedb::reducer]
pub fn set_stalwart_config(
    ctx: &ReducerContext,
    jmap_url: String,
    admin_token: String,
) -> Result<(), String> {
    if !is_admin_identity(ctx, ctx.sender()) {
        return Err(format!("Unauthorized: {:?}", ctx.sender()));
    }

    let trimmed_url = jmap_url.trim().trim_end_matches('/').to_string();
    if trimmed_url.is_empty() {
        return Err("JMAP URL cannot be empty".to_string());
    }
    if admin_token.trim().is_empty() {
        return Err("Admin token cannot be empty".to_string());
    }

    if let Some(mut existing) = ctx.db.stalwart_config().id().find(&0) {
        existing.jmap_url = trimmed_url;
        existing.admin_token = admin_token;
        existing.updated_at = ctx.timestamp;
        ctx.db.stalwart_config().id().update(existing);
    } else {
        ctx.db.stalwart_config().insert(StalwartConfig {
            id: 0,
            jmap_url: trimmed_url,
            admin_token,
            updated_at: ctx.timestamp,
        });
    }

    Ok(())
}
