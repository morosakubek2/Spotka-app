#!/bin/bash
# Skrypt budowania wersji Alpha dla Androida i iOS

set -e

echo "🚀 Rozpoczynanie budowania Spotka v0.1.0-alpha..."

# Sprawdzenie czy Rust jest zainstalowany
if ! command -v cargo &> /dev/null; then
    echo "❌ Błąd: Cargo nie zostało znalezione. Zainstaluj Rust."
    exit 1
fi

# Budowanie rdzenia dla Androida
echo "📱 Budowanie rdzenia Rust dla Androida..."
cd mobile/rust-core
cargo ndk -t arm64-v8a -t armeabi-v7a -o ../android/app/src/main/jniLibs build --release
cd ../..

# Budowanie APK dla Androida
echo "🤖 Generowanie APK..."
cd mobile/android
chmod +x gradlew
./gradlew assembleRelease
echo "✅ APK gotowe: mobile/android/app/build/outputs/apk/release/app-release.apk"
cd ../..

# Budowanie rdzenia dla iOS (tylko na macOS)
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "🍎 Budowanie rdzenia Rust dla iOS..."
    cd mobile/rust-core
    cargo build --release --target aarch64-apple-ios
    mkdir -p ../ios/Frameworks
    cp target/aarch64-apple-ios/release/libspotka_core.a ../ios/Frameworks/
    cd ../..

    echo "📦 Generowanie IPA..."
    cd mobile/ios
    xcodebuild -workspace Spotka.xcworkspace \
        -scheme Spotka \
        -configuration Release \
        -destination 'generic/platform=iOS' \
        -archivePath $PWD/build/Spotka.xcarchive \
        archive
    echo "✅ IPA gotowe: mobile/ios/build/Spotka.xcarchive"
    cd ../..
else
    echo "⚠️ Pomijanie budowania iOS (wymagany macOS)."
fi

echo "🎉 Budowanie zakończone sukcesem!"
echo "📂 Artefakty:"
echo "   - Android: mobile/android/app/build/outputs/apk/release/"
echo "   - iOS: mobile/ios/build/"
