use spacetimedb::ReducerContext;

pub fn make_ingress_id(ctx: &ReducerContext, queue_id: &str, category_id: u64) -> String {
    let entropy = ctx.random::<u128>();
    format!("{queue_id}:{category_id}:{entropy:032x}")
}

pub fn make_delivery_id(
    ingress_id: &str,
    subscription_id: u64,
    recipient_email: &str,
) -> String {
    format!("{ingress_id}:{subscription_id}:{recipient_email}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_delivery_id() {
        let id = make_delivery_id("ing-123", 42, "user@example.com");
        assert_eq!(id, "ing-123:42:user@example.com");
    }
}
