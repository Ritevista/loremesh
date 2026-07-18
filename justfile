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

corpus-fixture:
    cargo run -p loremesh -- workspace init target/test-corpora/fixture-workspace
    cd target/test-corpora/fixture-workspace && ../../debug/loremesh corpus import ../../../tests/fixtures/knowledge-base/corpus.json
    cd target/test-corpora/fixture-workspace && ../../debug/loremesh index build knowledge

corpus-public:
    cargo run -p loremesh-public-corpus -- verify-profile
    cargo run -p loremesh-public-corpus -- build --output target/test-corpora/kubernetes-feature-sample

corpus-public-verify:
    cargo run -p loremesh-public-corpus -- verify-profile

corpus-scale-100m:
    cargo run -p loremesh-corpus-gen -- --seed 42 --documents 1000 --issues 500 --relationships 5000 --target-size 100MB --quality-problems --allow-large --output target/test-corpora/scale-100m

corpus-scale-500m:
    cargo run -p loremesh-corpus-gen -- --seed 42 --documents 5000 --issues 2500 --relationships 25000 --target-size 500MB --quality-problems --allow-large --output target/test-corpora/scale-500m

corpus-scale-1g:
    cargo run -p loremesh-corpus-gen -- --seed 42 --documents 10000 --issues 5000 --relationships 50000 --target-size 1GB --quality-problems --allow-large --output target/test-corpora/scale-1g

corpus-scale-2g:
    cargo run -p loremesh-corpus-gen -- --seed 42 --documents 20000 --issues 10000 --relationships 100000 --target-size 2GB --quality-problems --allow-large --output target/test-corpora/scale-2g

corpus-clean:
    rm -rf target/test-corpora
