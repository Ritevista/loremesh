pub fn calculate_delay(attempt: u32) -> u64 {
    u64::from(attempt.min(3)) * 100
}

