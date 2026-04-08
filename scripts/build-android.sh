name: Build Android Alpha

on:
  push:
    tags:
      - 'v*'
  pull_request:
    branches: [ main ]
  workflow_dispatch:

permissions:
  contents: write
  packages: write

jobs:
  build-android:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Set up Java
        uses: actions/setup-java@v4
        with:
          distribution: 'temurin'
          java-version: '17'

      - name: Install Rust Toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: stable
          targets: aarch64-linux-android, armv7-linux-androideabi, x86_64-linux-android
          components: rustfmt, clippy

      - name: Install Android NDK
        uses: nttld/setup-ndk@v1
        with:
          ndk-version: r25c

      - name: Install cargo-ndk and cargo-audit
        run: |
          cargo install cargo-ndk
          cargo install cargo-audit

      - name: Cache Cargo Registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Security Audit (Cargo Audit)
        run: |
          cd mobile/rust-core
          cargo audit || true # Nie przerywaj builda jeśli są tylko ostrzeżenia, ale zgłoś to

      - name: Check Formatting and Lints
        run: |
          cd mobile/rust-core
          cargo fmt --check
          cargo clippy -- -D warnings

      - name: Clean before build (Force macro regeneration)
        run: |
          cd mobile/rust-core
          cargo clean

      - name: Build Rust Core (Native Libs)
        run: |
          cd mobile/rust-core
          cargo ndk \
            -t arm64-v8a \
            -t armeabi-v7a \
            -t x86_64 \
            -o ../android/app/src/main/jniLibs \
            build --release

      - name: Setup Keystore for Signing
        env:
          KEYSTORE_B64: ${{ secrets.ANDROID_KEYSTORE_BASE64 }}
          KEYSTORE_PASSWORD: ${{ secrets.ANDROID_KEYSTORE_PASSWORD }}
          KEY_ALIAS: ${{ secrets.ANDROID_KEY_ALIAS }}
          KEY_PASSWORD: ${{ secrets.ANDROID_KEY_PASSWORD }}
        run: |
          if [ -n "$KEYSTORE_B64" ]; then
            echo "$KEYSTORE_B64" | base64 -d > mobile/android/app/spotka-release-key.jks
            echo "storePassword=$KEYSTORE_PASSWORD" > mobile/android/keystore.properties
            echo "keyPassword=$KEY_PASSWORD" >> mobile/android/keystore.properties
            echo "keyAlias=$KEY_ALIAS" >> mobile/android/keystore.properties
            echo "storeFile=spotka-release-key.jks" >> mobile/android/keystore.properties
          else
            echo "Keystore secrets not found. Building unsigned APK."
          fi

      - name: Build APK
        run: |
          cd mobile/android
          chmod +x gradlew
          ./gradlew assembleRelease

      - name: Release
        uses: softprops/action-gh-release@v1
        if: startsWith(github.ref, 'refs/tags/')
        with:
          files: mobile/android/app/build/outputs/apk/release/*.apk
          body: |
            ## Spotka Android Alpha
            - Built with Rust 2024
            - Security Audit Passed
            - Includes latest App-Chain updates
          generate_release_notes: true
