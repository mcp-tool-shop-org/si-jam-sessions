#!/usr/bin/env sh
# Every gate CI's rust job runs, in one command, stopping at the first failure:
# format, lints, tests, the wasm law, licences and RustSec advisories, and the goldens.
# Needs the pinned toolchain (rust-toolchain.toml) and cargo-deny 0.20.2.
set -eu
cd "$(dirname "$0")"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build -p law --target wasm32-unknown-unknown --release --locked
cargo deny --locked check licenses advisories
cargo run --locked -q -p golden --bin write-golden -- --check

echo "verify: every gate passed"
