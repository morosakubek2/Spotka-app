# Contributing to Spotka

Thank you for your interest in contributing to **Spotka**! 
Spotka is an "Anti-Social", decentralized, peer-to-peer meetup planner built with **100% Rust**. 
Our goal is to facilitate physical human interaction while maximizing privacy and security.

Before contributing, please read this guide carefully to understand our philosophy, technical requirements, and workflow.

## 🛑 Philosophy & Code of Conduct

1.  **Anti-Social by Design**: We do **not** accept features that add chat, social feeds, likes, or digital distractions. The app is a tool for organizing *physical* meetups, not a social network.
2.  **Privacy First**: No user data (phone numbers, locations, identities) should ever be sent to a central server in plain text. All sensitive data must be encrypted locally (SQLCipher) and transmitted only via E2EE (P2P).
3.  **Zero-Knowledge Architecture**: Developers must respect the principle that the core team knows nothing about users. Do not introduce backdoors or telemetry.
4.  **Inclusivity**: Be respectful. We welcome contributors from all backgrounds, provided they adhere to our technical and ethical standards.

## 🛠️ Getting Started

### Prerequisites
- **Rust**: Edition 2024 (Latest Stable).
- **Android**: Android Studio, NDK r25+, `cargo-ndk`.
- **iOS**: Xcode 15+, `cargo-lipo` (or standard `cargo build` for xcframework).
- **Slint**: Familiarity with Slint UI framework (Reductive Functionalism style).
- **Git**: For version control.

### Setup
1.  Fork the repository.
2.  Clone your fork:
    ```bash
    git clone https://github.com/YOUR_USERNAME/spotka.git
    cd spotka
    ```
3.  Initialize submodules (if any) and install tools:
    ```bash
    rustup update
    cargo install cargo-ndk cargo-audit
    ```
4.  Run the setup script:
    ```bash
    ./scripts/setup_dev.sh
    ```

## 📝 How to Contribute

### 1. Reporting Bugs
- Check existing issues first.
- Use the **Bug Report Template**.
- **Security Bugs**: Do NOT open a public issue. Email `security@spotka.app` or use GitHub Private Vulnerability Reporting.
- Include: Steps to reproduce, expected vs. actual behavior, logs (sanitized!), device/OS version.

### 2. Suggesting Features
- Ensure the feature aligns with the **Anti-Social Philosophy**.
- Features adding centralization, chat, or tracking will be rejected.
- Good candidates: Performance optimizations, new P2P transport protocols, UI accessibility improvements, new dictionary languages.
- Open a **Feature Request** issue with a clear rationale.

### 3. Pull Requests (PRs)
1.  Create a branch from `main`:
    ```bash
    git checkout -b feat/your-feature-name
    ```
2.  Make your changes. Follow our **Coding Standards**.
3.  Write tests! (Unit tests for logic, integration tests for P2P/DB).
4.  Run linters and auditors:
    ```bash
    cargo fmt --check
    cargo clippy -- -D warnings
    cargo audit
    ```
5.  Commit with meaningful messages (see below).
6.  Push and open a PR against `main`.

## 💻 Coding Standards

### Rust Guidelines
- **Edition**: Must use Rust 2024.
- **Safety**: Avoid `unsafe` blocks unless absolutely necessary (e.g., FFI). Justify every `unsafe` block with comments.
- **Error Handling**: Use `Result<T, E>` extensively. **NO PANICS** in production code.
- **Language Agnostic**: 
  - **NEVER** hardcode strings (English, Polish, etc.) in logic or UI.
  - Use keys (e.g., `"ERR_DB_LOCKED"`) and rely on the `dict` module for translation.
- **Memory Security**: Use `zeroize` for clearing sensitive data (keys, passwords) from memory.
- **Async**: Use `tokio` for async runtime. Keep async boundaries clean.

### UI Guidelines (Slint)
- **Style**: "Reductive Functionalism". Black & White only. No colors unless critical for accessibility (and even then, use patterns/thickness).
- **Components**: Reuse components (`tag_badge`, `user_card`). Do not duplicate code.
- **Performance**: Keep bindings light. Heavy logic belongs in Rust, not `.slint` expressions.

### Testing
- Unit tests are mandatory for core logic (`crypto`, `chain`, `dict`).
- Integration tests for `p2p` and `db` modules.
- Tests must pass in CI before merging.

## 🔒 Security Requirements

- **Cryptography**: Use established crates (`ed25519-dalek`, `aes-gcm`, `argon2`). Do not roll your own crypto.
- **Dependencies**: All new dependencies must be vetted. `cargo audit` must pass.
- **Secrets**: Never commit secrets, keys, or `.env` files.
- **FFI**: Be extremely careful with pointer safety in `ffi/android.rs` and `ffi/ios.rs`.

## 📜 Developer Certificate of Origin (DCO)

By contributing to Spotka, you agree that your contributions are licensed under the project's license (AGPL-3.0) and that you have the right to submit them. 
We require a sign-off on commits:
```bash
git commit -s -m "feat: add new CTS parser optimization"
