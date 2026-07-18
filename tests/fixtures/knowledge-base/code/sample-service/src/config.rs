pub fn is_known_key(key: &str) -> bool {
    matches!(key, "relay.batch_size" | "rollout.pause")
}

