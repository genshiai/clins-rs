#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --locked -p clins-ts --target wasm32-unknown-unknown --release
wasm-bindgen --target web --typescript --out-dir bindings/typescript/pkg target/wasm32-unknown-unknown/release/clins_ts.wasm
