#!/bin/bash
set -e # Exit immediately if a command exits with a non-zero status.

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 Starting Android Build Script for Spotka...${NC}"

# 1. Check Prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: cargo is not installed. Please install Rust.${NC}"
    exit 1
fi

if ! command -v cargo-ndk &> /dev/null; then
    echo -e "${YELLOW}cargo-ndk not found. Installing...${NC}"
    cargo install cargo-ndk
fi

# Define paths
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ROOT_DIR="$SCRIPT_DIR/.."
RUST_CORE_DIR="$ROOT_DIR/mobile/rust-core"
ANDROID_JNI_LIBS_DIR="$ROOT_DIR/mobile/android/app/src/main/jniLibs"

# 2. Clean old artifacts
echo -e "${YELLOW}Cleaning old native libraries...${NC}"
rm -rf "$ANDROID_JNI_LIBS_DIR"/*

# 3. Add Targets if missing
echo -e "${YELLOW}Ensuring Rust targets are installed...${NC}"
rustup target add aarch64-linux-android
rustup target add armv7-linux-androideabi
rustup target add x86_64-linux-android
rustup target add i686-linux-android

# 4. Build Rust Core for Android
echo -e "${GREEN}Building Rust Core (this may take a while)...${NC}"
cd "$RUST_CORE_DIR"

# Determine build type (default to release if not specified)
BUILD_TYPE="${1:-release}"
CARGO_ARGS=""
if [ "$BUILD_TYPE" == "release" ]; then
    CARGO_ARGS="--release"
fi

# Execute cargo-ndk
# Output directory structure matches Android expectations automatically
cargo ndk \
    -t arm64-v8a \
    -t armeabi-v7a \
    -t x86_64 \
    -o "$ANDROID_JNI_LIBS_DIR" \
    build $CARGO_ARGS

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Rust Core built successfully!${NC}"
    echo -e "${GREEN}Libraries placed in: $ANDROID_JNI_LIBS_DIR${NC}"
else
    echo -e "${RED}❌ Build failed!${NC}"
    exit 1
fi

# 5. Next Steps Hint
echo -e "${YELLOW}--------------------------------------------------${NC}"
echo -e "${YELLOW}Next step: Run './gradlew assembleDebug' or './gradlew assembleRelease' in mobile/android/${NC}"
echo -e "${YELLOW}--------------------------------------------------${NC}"
