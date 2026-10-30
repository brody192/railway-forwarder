#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
export MACOSX_DEPLOYMENT_TARGET=26.0
export CARGO_HOME="$PWD/.build/cargo-home"
export CARGO_TARGET_DIR="$PWD/.build/rust"
if [[ -d "$PWD/.build/rustup/toolchains/stable-aarch64-apple-darwin" ]]; then
  export RUSTUP_HOME="$PWD/.build/rustup"
  export RUSTUP_TOOLCHAIN=stable
fi
cargo test --workspace --locked --release "$@"
