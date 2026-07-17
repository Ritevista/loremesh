set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

check:
    cargo fmt --all --check
    cargo check --workspace --all-targets --all-features
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    ./scripts/check-architecture.sh

test:
    cargo test --workspace --all-targets --all-features
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

ci: check test
    cargo deny check

demo:
    cargo run -p loremesh -- workspace init target/demo-workspace
    cd target/demo-workspace && ../debug/loremesh demo seed
    cd target/demo-workspace && ../debug/loremesh workspace status
    cd target/demo-workspace && ../debug/loremesh report export --format html --output report.html

coverage:
    cargo llvm-cov --workspace --all-features --html
