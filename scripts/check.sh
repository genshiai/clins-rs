#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
python3 scripts/check-versions.py
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --locked -p clins-core -p clins-packages -p clins-cli --all-targets
cargo run --locked -p clins-cli -- --packages clins-packages.lock.json prepare
cargo test --locked -p clins-cli --test clins --test synthetic -- --ignored
cargo run --locked -p clins-cli -- --packages clins-packages.lock.json export-definitions > data/definitions.json
