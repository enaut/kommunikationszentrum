use spacetimedb::{ReducerContext, Table, Timestamp};

use crate::common::delivery_types::ClaimState;
use crate::common::id::make_ingress_id;
use crate::models::delivery::{MailIngress, mail_ingress};
use crate::models::mail_message::mail_message;

pub(crate) fn upsert_mail_ingress(
    ctx: &ReducerContext,
    mail_message_id: u64,
    topic_id: u64,
    topic_email: String,
) -> String {
    let msg = ctx
        .db
        .mail_message()
        .id()
        .find(&mail_message_id)
        .expect("MailMessage must exist before MailIngress");
    let queue_id = msg.queue_id.as_deref().unwrap_or("");
    let ingress_id = make_ingress_id(ctx, queue_id, topic_id);

    ctx.db.mail_ingress().insert(MailIngress {
        id: ingress_id.clone(),
        mail_message_id,
        topic_id,
        topic_email,
        claim: ClaimState::new_pending(ctx.timestamp),
        recipient_count: 0,
        delivery_count: 0,
        failed_delivery_count: 0,
        completed_at: Timestamp::UNIX_EPOCH,
    });

    ingress_id
}
