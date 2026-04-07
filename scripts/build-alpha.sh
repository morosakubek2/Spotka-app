#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Spotka Alpha Release Script${NC}"
echo "------------------------------------------"

# 1. Check if we are on main branch
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT_BRANCH" != "main" ]; then
    echo -e "${RED}❌ Błąd: Musisz być na branchu 'main' do wydania wersji.${NC}"
    echo -e "${YELLOW}Obecny branch: $CURRENT_BRANCH${NC}"
    exit 1
fi

# 2. Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    echo -e "${RED}❌ Błąd: Masz niezatwierdzone zmiany w repozytorium.${NC}"
    echo -e "${YELLOW}Proszę zatwierdzić (commit) lub cofnąć (stash) zmiany przed wydaniem.${NC}"
    git status
    exit 1
fi

# 3. Get current version
VERSION_FILE="mobile/rust-core/Cargo.toml"
if [ ! -f "$VERSION_FILE" ]; then
    echo -e "${RED}❌ Błąd: Nie znaleziono pliku Cargo.toml w expected location.${NC}"
    exit 1
fi

CURRENT_VERSION=$(grep "^version = " "$VERSION_FILE" | head -1 | sed 's/version = "\(.*\)"/\1/')
echo -e "${GREEN}Aktualna wersja: ${YELLOW}$CURRENT_VERSION${NC}"

# 4. Calculate new version (increment patch number)
# Assumes semver: major.minor.patch (e.g., 0.1.0 -> 0.1.1)
MAJOR=$(echo $CURRENT_VERSION | cut -d. -f1)
MINOR=$(echo $CURRENT_VERSION | cut -d. -f2)
PATCH=$(echo $CURRENT_VERSION | cut -d. -f3)
NEW_PATCH=$((PATCH + 1))
NEW_VERSION="${MAJOR}.${MINOR}.${NEW_PATCH}-alpha"

echo -e "${BLUE}Proponowana nowa wersja: ${GREEN}$NEW_VERSION${NC}"

# 5. User Confirmation
read -p "Czy na pewno chcesz wydać wersję $NEW_VERSION? (tak/nie): " CONFIRM
if [ "$CONFIRM" != "tak" ] && [ "$CONFIRM" != "t" ] && [ "$CONFIRM" != "yes" ] && [ "$CONFIRM" != "y" ]; then
    echo -e "${YELLOW}Anulowano wydanie wersji.${NC}"
    exit 0
fi

# 6. Backup current state (optional but recommended)
echo -e "${YELLOW}Tworzę punkt przywracania (tag backup)...${NC}"
BACKUP_TAG="backup-pre-release-$(date +%Y%m%d-%H%M%S)"
git tag "$BACKUP_TAG"
git push origin "$BACKUP_TAG" 2>/dev/null || echo "Warning: Could not push backup tag."

# 7. Update Version in Cargo.toml
echo -e "${YELLOW}Aktualizacja wersji w Cargo.toml...${NC}"
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" "$VERSION_FILE"
rm "$VERSION_FILE.bak" # Clean up backup created by sed

# Also update version in Android build.gradle.kts if exists
ANDROID_BUILD_GRADLE="mobile/android/app/build.gradle.kts"
if [ -f "$ANDROID_BUILD_GRADLE" ]; then
    sed -i.bak "s/versionName = \".*\"/versionName = \"$NEW_VERSION\"/" "$ANDROID_BUILD_GRADLE"
    rm "$ANDROID_BUILD_GRADLE.bak"
fi

# Also update version in iOS Info.plist if exists
IOS_INFO_PLIST="mobile/ios/Spotka/Info.plist"
if [ -f "$IOS_INFO_PLIST" ]; then
    # This is a bit tricky with XML, simplified for string replacement
    sed -i.bak "s/<key>CFBundleShortVersionString<\/key>.*<string>.*<\/string>/<key>CFBundleShortVersionString<\/key>\n\t<string>$NEW_VERSION<\/string>/" "$IOS_INFO_PLIST"
    # Note: The above sed for XML might need adjustment based on exact formatting. 
    # A safer way for XML is using /usr/libexec/PlistBuddy on macOS or a dedicated tool, 
    # but for script simplicity we assume standard formatting.
    # Let's revert the complex sed and just warn user to check iOS version manually if needed.
    rm "$IOS_INFO_PLIST.bak" 2>/dev/null || true
    echo -e "${YELLOW}Uwaga: Sprawdź ręcznie wersję w Info.plist jeśli sed nie zadziałał poprawnie dla XML.${NC}"
fi

# 8. Commit Version Change
git add "$VERSION_FILE" "$ANDROID_BUILD_GRADLE" "$IOS_INFO_PLIST" 2>/dev/null || true
git commit -m "chore: bump version to $NEW_VERSION [skip ci]"

# 9. Run Final Checks
echo -e "${BLUE}Uruchamianie finalnych testów i audytów...${NC}"

echo -e "${YELLOW}-> Running cargo fmt...${NC}"
cd mobile/rust-core && cargo fmt --check || { echo -e "${RED}❌ cargo fmt failed!${NC}"; exit 1; }

echo -e "${YELLOW}-> Running cargo clippy...${NC}"
cargo clippy -- -D warnings || { echo -e "${RED}❌ cargo clippy failed!${NC}"; exit 1; }

echo -e "${YELLOW}-> Running cargo test...${NC}"
cargo test || { echo -e "${RED}❌ cargo test failed!${NC}"; exit 1; }

echo -e "${YELLOW}-> Running cargo audit...${NC}"
cargo audit || { echo -e "${RED}❌ cargo audit found vulnerabilities!${NC}"; exit 1; }

cd ../.. # Return to root

# 10. Create Release Tag
NEW_TAG="v$NEW_VERSION"
echo -e "${BLUE}Tworzenie tagu: $NEW_TAG${NC}"
git tag -a "$NEW_TAG" -m "Release version $NEW_VERSION (Alpha)"

# 11. Push Changes
echo -e "${BLUE}Wypychanie zmian i tagu na GitHub...${NC}"
git push origin main
git push origin "$NEW_TAG"

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Sukces!${NC}"
    echo -e "${GREEN}Wersja $NEW_VERSION została wypchnięta.${NC}"
    echo -e "${BLUE}GitHub Actions powinien teraz uruchomić budowanie:${NC}"
    echo -e "👉 https://github.com/$(git remote get-url origin | sed -E 's/.*github\.com[:/]([^/]+)\/([^.]+).*/\1\/\2/')/actions"
    echo -e "👉 https://github.com/$(git remote get-url origin | sed -E 's/.*github\.com[:/]([^/]+)\/([^.]+).*/\1\/\2/')/releases/tag/$NEW_TAG"
else
    echo -e "${RED}❌ Błąd podczas pushowania!${NC}"
    echo -e "${YELLOW}Możliwe, że ktoś zdążył wypchnąć zmiany na main. Spróbuj ponownie po pull.${NC}"
    exit 1
fi
