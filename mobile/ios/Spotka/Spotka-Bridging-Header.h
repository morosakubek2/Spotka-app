// mobile/ios/Spotka/Spotka-Bridging-Header.h
// Bridging Header for Spotka Rust Core (FFI).
// Enables Swift to call Rust functions via C-compatible interface.
// Year: 2026

#ifndef Spotka_Bridging_Header_h
#define Spotka_Bridging_Header_h

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

// --- Core Lifecycle ---

/// Initializes the Spotka core runtime, logger, and global state.
/// Must be called once at app launch before any other function.
void spotka_init(void);

/// Returns the current version string of the Rust core.
/// Caller is responsible for freeing the returned string using spotka_free_string.
const char *spotka_get_version(void);

// --- Identity & Crypto ---

/// Generates or loads the user identity based on the provided phone number hash.
/// Returns a JSON string with public key info or an error code.
/// Caller must free the result.
const char *spotka_identity_init(const char *phone_hash);

/// Signs a data buffer with the user's private key.
/// Returns a new buffer containing the signature.
/// Caller must free the result using spotka_free_buffer.
uint8_t *spotka_sign_data(const uint8_t *data, size_t len, size_t *out_len);

// --- Database (SQLCipher) ---

/// Opens or creates the encrypted database at the given path.
/// auth_token is used to derive the encryption key (e.g., from Biometry/Keychain).
/// Returns 0 on success, error code otherwise.
int32_t spotka_db_open(const char *db_path, const char *auth_token);

/// Closes the database connection safely.
void spotka_db_close(void);

// --- P2P Network ---

/// Starts the P2P node in the background.
/// mode: 0=Eco, 1=Active, 2=Guardian.
void spotka_p2p_start(int32_t mode);

/// Stops the P2P node and releases network resources.
void spotka_p2p_stop(void);

/// Broadcasts a message to the network.
/// topic: string identifier (e.g., "meetup_update").
/// payload: binary data to send.
void spotka_p2p_broadcast(const char *topic, const uint8_t *payload, size_t len);

// --- Memory Management Helpers ---
/// Crucial for preventing memory leaks when passing strings/buffers from Rust to Swift.

/// Frees a C-string allocated by Rust.
void spotka_free_string(const char *str);

/// Frees a byte buffer allocated by Rust.
void spotka_free_buffer(uint8_t *buf);

#endif /* Spotka_Bridging_Header_h */
