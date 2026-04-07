# 🚀 Spotka - Anty-społecznościowa aplikacja P2P

**Wersja:** v0.1.0-alpha  
**Licencja:** AGPL-3.0 (Klient), Commercial (Serwer Premium)  
**Status:** Code Complete - Gotowe do testów Alpha

## 📱 Opis
Spotka to zdecentralizowana aplikacja mobilna do planowania fizycznych spotkań, działająca w architekturze P2P bez centralnych serwerów (wersja Free). Zero feedów, zero lajków, zero czatów – tylko realne spotkania.

## 🔧 Wymagania
- Rust (stable)
- Android NDK r25c+ (dla Androida)
- Xcode 15+ (dla iOS, wymagany macOS)
- cargo-ndk, cargo-lipo

## 🛠️ Budowanie

### Automatyczne (GitHub Actions)
Aplikacja buduje się automatycznie po wypchnięciu tagu wersji:
```bash
git tag v0.1.0-alpha
git push origin v0.1.0-alpha
```
Workflow uruchomi się w zakładce **Actions**, a gotowe pliki pojawią się w **Releases**.

### Ręczne
Uruchom skrypt budujący:
```bash
./scripts/build-alpha.sh
```

## 📦 Struktura projektu
```
/workspace
├── .github/workflows/    # CI/CD dla Android/iOS
├── mobile/
│   ├── rust-core/        # Rdzeń aplikacji w Rust
│   ├── android/          # Projekt Android (Kotlin)
│   └── ios/              # Projekt iOS (Swift)
├── scripts/              # Skrypty pomocnicze
└── README.md
```

## 🌟 Funkcje Alpha
- ✅ Pełna decentralizacja P2P (libp2p)
- ✅ Szyfrowanie E2EE (Ed25519, X25519, AES-GCM)
- ✅ System tagów CTS z kompresją hybrydową
- ✅ Web of Trust z weryfikacją SMS
- ✅ Wielojęzyczny interfejs (Slint)
- ✅ Adaptacyjne zarządzanie energią
- ✅ App-Chain z samoczyszczeniem danych

## 📝 Instrukcja dla testerów
1. Pobierz APK/IPA z zakładki Releases.
2. Zainstaluj na urządzeniu (Android: włącz "Nieznane źródła", iOS: podpis developerski).
3. Przy pierwszym uruchomieniu wpisz swój numer telefonu.
4. Aby dodać znajomego, spotkajcie się fizycznie i użyjcie sekcji [PING] do wymiany kodów QR.
5. Stwórz pierwsze spotkanie w sekcji [SPOTKANIA].

## ⚠️ Uwagi
- Brak kopii zapasowej = utrata wszystkich danych i reputacji.
- Wymagana fizyczna obecność do nawiązywania relacji.
- Tryb eksperymentalny (customowe słowniki) domyślnie wyłączony.

## 🤝 Współpraca
Projekt Open Core. Zapraszam do zgłaszania błędów przez GitHub Issues.
