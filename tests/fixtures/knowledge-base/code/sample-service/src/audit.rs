pub fn next_sequence(current: u64) -> Option<u64> {
    current.checked_add(1)
}

