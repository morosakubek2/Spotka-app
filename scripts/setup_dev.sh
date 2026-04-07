#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 Initializing Spotka Development Environment...${NC}"

# 1. Check Prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

if ! command -v rustc &> /dev/null; then
    echo -e "${RED}❌ Rust is not installed. Please install it from https://rustup.rs${NC}"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Cargo is not installed.${NC}"
    exit 1
fi

# 2. Install Rust Tools
echo -e "${YELLOW}Installing Rust tools...${NC}"

if ! command -v cargo-ndk &> /dev/null; then
    echo "Installing cargo-ndk..."
    cargo install cargo-ndk
else
    echo "cargo-ndk already installed."
fi

if ! command -v cargo-audit &> /dev/null; then
    echo "Installing cargo-audit..."
    cargo install cargo-audit
else
    echo "cargo-audit already installed."
fi

if ! command -v cargo-lipo &> /dev/null; then
    # Only needed for older iOS setups, but good to have
    echo "Installing cargo-lipo..."
    cargo install cargo-lipo
else
    echo "cargo-lipo already installed."
fi

# 3. Add Targets
echo -e "${YELLOW}Adding Rust targets...${NC}"

# Android Targets
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android 2>/dev/null || true

# iOS Targets
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim 2>/dev/null || true

# 4. Setup Pre-commit Hooks
echo -e "${YELLOW}Setting up pre-commit hooks...${NC}"
mkdir -p .git/hooks

cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
echo "Running pre-commit checks..."
cargo fmt --check || { echo "❌ Formatting failed. Run 'cargo fmt'."; exit 1; }
cargo clippy -- -D warnings || { echo "❌ Clippy found warnings. Fix them."; exit 1; }
echo "✅ Checks passed."
EOF

chmod +x .git/hooks/pre-commit

# 5. Create Necessary Empty Files/Dirs
echo -e "${YELLOW}Creating local configuration files...${NC}"

# Android local properties (placeholder)
if [ ! -f mobile/android/local.properties ]; then
    echo "# sdk.dir=/path/to/Android/sdk" > mobile/android/local.properties
    echo "⚠️ Created mobile/android/local.properties. Please edit it with your SDK path."
fi

# Empty dictionary placeholders if missing
mkdir -p assets/dicts
touch assets/dicts/user_custom.json
touch assets/dicts/local_cache.json

# Create empty DB file placeholder (ignored by git)
touch spotka_data.db.tmp

# 6. Final Checks
echo -e "${GREEN}✅ Environment setup complete!${NC}"
echo ""
echo "Next steps:"
echo "1. Edit mobile/android/local.properties with your Android SDK path."
echo "2. Run './scripts/build-android.sh' to build the Android app."
echo "3. Open mobile/ios/Spotka.xcworkspace in Xcode."
echo ""
echo "Happy coding! 🦀"
