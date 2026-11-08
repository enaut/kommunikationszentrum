use spacetimedb::ReducerContext;

pub fn make_ingress_id(ctx: &ReducerContext, queue_id: &str, topic_id: u64) -> String {
    let entropy = ctx.random::<u128>();
    format!("{queue_id}:{topic_id}:{entropy:032x}")
}
