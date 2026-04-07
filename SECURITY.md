# Spotka 🤝

**The Anti-Social Meetup Planner.**  
No chats. No feeds. No distractions. Just physical meetings.

[![Build Status](https://github.com/spotka-app/spotka/workflows/Build%20Android/badge.svg)](https://github.com/spotka-app/spotka/actions)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust Edition](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org)

---

## 🌟 Philosophy

Spotka is designed for people who want to meet in real life, not online. It strips away all "social" features that keep you glued to the screen:
- ❌ **No Chat**: Communicate face-to-face.
- ❌ **No Feeds**: No scrolling through endless updates.
- ❌ **No Likes/Comments**: Your reputation is based on attendance and trust, not popularity.
- ✅ **P2P Architecture**: Decentralized, serverless (for free tier), and resilient.
- ✅ **Privacy First**: Your phone number is hashed locally. No central database of users.

> "Technology should bring us together physically, not isolate us digitally."

---

## 🛠 Architecture

Built with **100% Rust** for maximum performance, safety, and energy efficiency.

- **Core**: Rust (Edition 2024)
- **UI**: [Slint](https://slint.dev/) (Reductive Functionalism Design System)
- **Database**: [Drift](https://drift.rs/) + SQLCipher (End-to-End Encrypted Local Storage)
- **Networking**: `libp2p` (TCP/QUIC + BLE/mDNS for offline discovery)
- **Cryptography**: `ed25519-dalek`, `x25519-dalek`, `aes-gcm`, `argon2`
- **Mobile Bindings**: JNI (Android), FFI/Swift (iOS)

### Security Model
- **Identity**: Based on SHA-256 hash of your phone number. The raw number never leaves your device.
- **Data**: All local data is encrypted using keys derived from your biometric/PIN.
- **Trust**: Web-of-Trust model. You are verified by people you physically meet.
- **App-Chain**: A lightweight local ledger records trust transactions and reputation updates.

---

## 🚀 Getting Started

### Prerequisites
- Rust (Latest Stable, Edition 2024)
- Android Studio (for Android) or Xcode (for iOS)
- `cargo-ndk` (for Android builds)
- `slint-cpp` or `slint-interpreter` (if running on desktop)

### Clone & Build

```bash
git clone https://github.com/spotka-app/spotka.git
cd spotka

# Initialize submodules (if any)
git submodule update --init --recursive
