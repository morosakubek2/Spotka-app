#!/bin/bash
# scripts/build-ios.sh
# Build script for Spotka iOS (Rust Core + Xcode Project)
# Year: 2026 | Rust Edition: 2024

set -e

echo "🍎 START: Building Spotka iOS Core..."

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_CORE_DIR="$PROJECT_DIR/mobile/rust-core"
IOS_FRAMEWORKS_DIR="$PROJECT_DIR/mobile/ios/Frameworks"
IOS_PROJECT_DIR="$PROJECT_DIR/mobile/ios/Spotka"

# 1. Clean previous builds
echo "🧹 Cleaning old builds..."
rm -rf "$IOS_FRAMEWORKS_DIR/libspotka_core.xcframework"
rm -rf "$RUST_CORE_DIR/target/aarch64-apple-ios"
rm -rf "$RUST_CORE_DIR/target/x86_64-apple-ios"

# 2. Install Rust targets if missing
rustup target add aarch64-apple-ios
rustup target add x86_64-apple-ios
rustup target add aarch64-apple-ios-sim

# 3. Build Rust Core for Device (ARM64)
echo "🦀 Building for Device (ARM64)..."
cd "$RUST_CORE_DIR"
cargo build --release --target aarch64-apple-ios

# 4. Build Rust Core for Simulator (x86_64 & ARM64 for M1/M2 Macs)
echo "🦀 Building for Simulator (x86_64 & ARM64)..."
cargo build --release --target x86_64-apple-ios
cargo build --release --target aarch64-apple-ios-sim

# 5. Create Static Libraries structure
# We need to combine simulator architectures into one lib if desired, or keep separate.
# Here we create a simple structure for xcframework creation.

DEVICE_LIB="$RUST_CORE_DIR/target/aarch64-apple-ios/release/libspotka_core.a"
SIM_X86_LIB="$RUST_CORE_DIR/target/x86_64-apple-ios/release/libspotka_core.a"
SIM_ARM_LIB="$RUST_CORE_DIR/target/aarch64-apple-ios-sim/release/libspotka_core.a"

# 6. Generate XCFramework
echo "📦 Generating XCFramework..."
mkdir -p "$IOS_FRAMEWORKS_DIR"

xcodebuild -create-xcframework \
    -library "$DEVICE_LIB" \
    -library "$SIM_X86_LIB" \
    -library "$SIM_ARM_LIB" \
    -output "$IOS_FRAMEWORKS_DIR/libspotka_core.xcframework"

echo "✅ XCFramework created at: $IOS_FRAMEWORKS_DIR/libspotka_core.xcframework"

# 7. (Optional) Update Xcode Project Linking
# Note: Ideally, this is done once in Xcode GUI by adding the framework.
# This script ensures the file exists for Xcode to find.

echo "🚀 iOS Core Build Complete!"
echo "Next step: Open 'mobile/ios/Spotka.xcworkspace' in Xcode and run."
