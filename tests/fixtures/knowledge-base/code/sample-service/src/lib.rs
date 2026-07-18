pub mod audit;
pub mod config;
pub mod retry;
pub mod rollout;

pub fn acknowledge(sequence: u64) -> String {
    format!("ack-{sequence}")
}

