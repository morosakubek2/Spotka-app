# Spotka - Anti-Social Networking App

## 🎯 Cel Projektu
**Spotka** to anty-społecznościowa aplikacja mobilna służąca **wyłącznie** do planowania i realizacji fizycznych spotkań w określonym miejscu i czasie. 

### Kluczowe Zasady:
- ❌ **Brak** czatów, feedów, lajków, komentarzy
- ❌ **Brak** powiadomień angażujących emocjonalnie  
- ✅ **Minimalistyczny UI/UX** - energetycznie oszczędny
- ✅ **Prywatność** - pełna decentralizacja (wersja Free)
- ✅ **Fizyczne spotkania** - nawet dla dwójki przyjaciół!

---

## 🏗️ Architektura Systemu (Dual-Mode)

### 🔹 Wersja Darmowa (Free) – Pełna Decentralizacja P2P + App-Chain

```
┌─────────────────────────────────────────────────────────┐
│                    Urządzenie Użytkownika               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Lokalna    │  │   Crypto     │  │   P2P        │  │
│  │   Baza Danych│  │   Manager    │  │   Network    │  │
│  │   (SQLCipher)│  │  (Ed25519/   │  │   (mDNS +    │  │
│  │              │  │   AES-GCM)   │  │   WebSocket) │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│         │                  │                  │         │
│         └──────────────────┼──────────────────┘         │
│                            │                            │
│                  ┌─────────▼─────────┐                 │
│                  │   App-Chain       │                 │
│                  │   (Lekki Blockchain)│                │
│                  │   - Klucze publiczne│                │
│                  │   - Hashe certyfikatów│              │
│                  │   - Reputation Score│                │
│                  └───────────────────┘                 │
└─────────────────────────────────────────────────────────┘
           ↕ (P2P Sync - Gossip Protocol)
┌─────────────────────────────────────────────────────────┐
│                    Inne Urządzenia                      │
└─────────────────────────────────────────────────────────┘
```

**Czego NIE przechowuje App-Chain:**
- ❌ Treści spotkań
- ❌ Dane osobowe
- ❌ Historia lokalizacji
- ❌ Numery telefonów

### 🔹 Wersja Płatna (Premium) – Architektura Hybrydowa

```
┌─────────────────────────────────────────────────────────┐
│                    Klient Flutter                       │
│  ┌──────────────────────────────────────────────────┐  │
│  │  P2P (prywatne spotkania) + HTTPS/gRPC (Premium) │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
           │ HTTPS + gRPC
           ▼
┌─────────────────────────────────────────────────────────┐
│              Serwer Rust (Actix/Axum)                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  Globalne    │  │  Monetyzacja │  │  Zaszyfrowany│  │
│  │  Wyszukiwanie│  │  (Eventy +   │  │  Backup      │  │
│  │              │  │   Geofencing)│  │  (E2EE)      │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│  ┌──────────────┐  ┌──────────────┐                    │
│  │  Biometria   │  │  Walidacja   │                    │
│  │  (tęczówka/  │  │  bezpieczeństwa│                  │
│  │   dłoń)      │  │              │                    │
│  └──────────────┘  └──────────────┘                    │
└─────────────────────────────────────────────────────────┘
```

---

## 💰 Model Monetyzacji „Active Subscription"

Użytkownik może odblokować funkcje Premium **BEZ płatności**:

1. **Udział w sponsorowanych wydarzeniach**
   - Weryfikacja obecności: Geofencing + QR Code
   - Partnerzy: kawiarnie, coworkingi, eventy kulturalne

2. **System reputacji**
   - Regularne potwierdzane spotkania = wyższy Reliability Score
   - Wysoki score = darmowy dostęp do wybranych funkcji Premium

### Funkcje Premium:
- ✨ Biometria (tęczówka/dłoń)
- ✨ Wydarzenia publiczne
- ✨ Rozszerzona sieć P2P (poza siecią lokalną)
- ✨ Zaawansowane filtry reputacji
- ✨ Synchronizacja ustawień między urządzeniami
- ✨ Zaszyfrowany backup w chmurze

---

## 🛠️ Stack Technologiczny

### Frontend (Mobile)
- **Framework**: Flutter (Dart)
- **State Management**: Riverpod
- **Database**: Drift + SQLCipher (encrypted SQLite)
- **Cryptography**: 
  - `pointycastle` (Ed25519, X25519, AES-GCM)
  - `encrypt` (AES-GCM wrapper)
- **P2P Networking**:
  - `mdns` (lokalna discoverability)
  - `web_socket_channel` (direct communication)
- **Geolocation**: `geolocator`, `geofencing`
- **QR Codes**: `mobile_scanner`, `qr_flutter`
- **Biometrics**: `local_auth`
- **Routing**: `go_router`

### Backend (Premium - Rust)
- **Framework**: Actix-web / Axum
- **Protocol**: HTTPS + gRPC
- **Database**: PostgreSQL (encrypted at rest)
- **Authentication**: JWT + biometric verification

---

## 📁 Struktura Projektu

```
spotka_app/
├── lib/
│   ├── main.dart                    # Entry point
│   ├── core/
│   │   ├── crypto/
│   │   │   └── crypto_manager.dart  # Ed25519, AES-GCM, App-Chain
│   │   ├── p2p/
│   │   │   └── p2p_manager.dart     # mDNS, WebSocket, Gossip
│   │   └── database/
│   │       └── database.dart        # Drift schema (encrypted)
│   ├── features/
│   │   ├── meeting_planner/         # Planowanie spotkań
│   │   ├── reputation/              # System reputacji
│   │   └── auth/                    # Auth (biometric + keys)
│   └── shared/
│       ├── theme.dart               # Minimalist UI theme
│       ├── widgets/                 # Reusable components
│       └── utils/                   # Helpers
├── android/                         # Android-specific config
├── ios/                             # iOS-specific config
└── pubspec.yaml                     # Dependencies
```

---

## 🔐 Bezpieczeństwo i Prywatność

### Kryptografia
- **Tożsamość**: Ed25519 (klucze publiczno-prywatne)
- **Key Exchange**: X25519 (ECDH)
- **Szyfrowanie danych**: AES-256-GCM
- **Haszowanie**: SHA-256
- **Storage**: SQLCipher (encrypted SQLite)

### Web of Trust
- Certyfikaty zaufania wystawiane przez innych użytkowników
- Publiczne wskaźniki reputacji (Reliability Score 0-100)
- Możliwość odwołania certyfikatu

### Auto-usuwanie danych
- Spotkania automatycznie usuwane po upływie czasu
- Historia przechowywana lokalnie (konfigurowalne)
- Brak centralnego logowania lokalizacji

---

## 🚀 Następne Kroki

1. **Implementacja core**:
   - [x] Schema bazy danych (Drift)
   - [x] Crypto Manager (Ed25519, AES-GCM)
   - [x] P2P Manager (mDNS, WebSocket)
   - [ ] Integracja z Drift + SQLCipher
   - [ ] Build_runner generation

2. **Feature development**:
   - [ ] Ekran tworzenia spotkania
   - [ ] Geofencing + QR verification
   - [ ] System reputacji
   - [ ] Biometric auth (Premium)

3. **Backend (Rust)**:
   - [ ] Actix/Axum server scaffold
   - [ ] Global search endpoint
   - [ ] Monetization verification
   - [ ] Encrypted backup API

4. **Testing**:
   - [ ] Unit tests (crypto, database)
   - [ ] Integration tests (P2P sync)
   - [ ] E2E tests (meeting flow)

---

## 📝 Licencja

Projekt open-source (MIT License) - zapraszam do contribucji!
