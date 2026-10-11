#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo test --locked --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features
cargo build --locked --release
cargo build --locked
python3 scripts/os_faults.py
cargo package --locked --allow-dirty
