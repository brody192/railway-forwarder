#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
configuration="${1:-release}"
if [[ "$configuration" != release && "$configuration" != debug ]]; then
  printf 'Usage: %s [release|debug]\n' "$0" >&2
  exit 1
fi
sdk_version="$(xcrun --sdk macosx --show-sdk-version)"
if (( ${sdk_version%%.*} < 26 )); then
  printf 'macOS SDK 26 or later is required for native Liquid Glass controls. Selected SDK is %s.\n' "$sdk_version" >&2
  exit 1
fi
export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
export CARGO_HOME="$PWD/.build/cargo-home"
export CARGO_TARGET_DIR="$PWD/.build/rust"
export MACOSX_DEPLOYMENT_TARGET=26.0
# A project-local toolchain can be used without changing the machine's default.
if [[ -d "$PWD/.build/rustup/toolchains/stable-aarch64-apple-darwin" ]]; then
  export RUSTUP_HOME="$PWD/.build/rustup"
  export RUSTUP_TOOLCHAIN=stable
fi
rust_profile=dev
if [[ "$configuration" == release ]]; then rust_profile=release; fi
cargo rustc --package railway-forwarder --locked --profile "$rust_profile" -- \
  -C "link-arg=-Wl,-platform_version,macos,26.0,$sdk_version"
app="dist/Railway Forwarder.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
# Replace the executable by rename so an already-running instance keeps its inode.
cp "$CARGO_TARGET_DIR/$configuration/RailwayForwarder" "$app/Contents/MacOS/RailwayForwarder.new"
mv -f "$app/Contents/MacOS/RailwayForwarder.new" "$app/Contents/MacOS/RailwayForwarder"
rm -f "$app/Contents/Frameworks/libForwarderNative.dylib"
if [[ -d "$app/Contents/Frameworks" ]]; then rmdir "$app/Contents/Frameworks" 2>/dev/null || true; fi
cp apps/railway-forwarder/resources/Info.plist "$app/Contents/Info.plist"
cp apps/railway-forwarder/resources/icons/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
binary_sdk="$(xcrun vtool -show-build "$app/Contents/MacOS/RailwayForwarder" | awk '$1 == "sdk" { print $2; exit }')"
if [[ -z "$binary_sdk" ]] || (( ${binary_sdk%%.*} < 26 )); then
  printf 'The app must identify macOS SDK 26 or later for Liquid Glass.\n' >&2
  exit 1
fi
codesign --force --sign - --identifier com.brody.railway-forwarder "$app"
touch "$app"
printf 'Built %s/%s\n' "$PWD" "$app"
