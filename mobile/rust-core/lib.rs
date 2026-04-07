// mobile/rust-core/src/lib.rs
// Spotka Core Library - The heart of the decentralized meetup planner.
// Architecture: 100% Rust, Anti-Social (No chat/feeds), P2P First.
// Language Agnostic: All error messages and UI strings are returned as KEYs, not text.

#![allow(dead_code)]
#![allow(unused_imports)]

// --- Module Declarations ---
pub mod chain;      // App-Chain: Lightweight distributed ledger for trust metadata
pub mod crypto;     // Cryptography: Ed25519, X25519, AES-GCM, Argon2
pub mod db;         // Database: Drift ORM + SQLCipher encryption
pub mod dict;       // Dictionaries: CTS Parser, I18N loader, Compression
pub mod p2p;        // Networking: libp2p, QUIC, BLE discovery
pub mod ui;         // User Interface: Slint components (compiled via build.rs)
pub mod ffi;        // Foreign Function Interface: Bridges for Android (JNI) & iOS
pub mod sync;       // Sync Logic: Delta updates, Gossip protocol state
pub mod app_controller; // Global State Machine: Manages app lifecycle

// --- External Crates for FFI ---
use libc::c_char;
use std::ffi::CString;
use std::os::raw::c_void;

// --- Initialization ---

/// Initializes the Spotka core runtime.
/// Should be called once from the native platform (Android/iOS) on app start.
/// Sets up logging and global state.
#[no_mangle]
pub extern "C" fn spotka_init() {
    // Initialize logger (outputs to logcat on Android, os_log on iOS)
    // In production, this might be configured to be silent or file-only for privacy.
    let _ = env_logger::try_init();
    
    log::info!("SPOTKA_CORE_INIT_SUCCESS");
    log::info!("SPOTKA_VERSION_0_1_0_ALPHA");
}

/// Returns the current version of the core library as a C-string.
/// Memory must be freed by the caller using spotka_free_string.
#[no_mangle]
pub extern "C" fn spotka_get_version() -> *const c_char {
    CString::new("0.1.0-alpha").unwrap().into_raw()
}

// --- Identity & Crypto Operations ---

/// Creates a new user identity based on a phone number.
/// NO SMS is sent. The phone number is hashed (SHA256) to create a unique, anonymous ID.
/// Returns a pointer to the serialized public key blob.
/// 
/// # Safety
/// The caller must ensure `phone_number_c_str` is a valid UTF-8 string.
#[no_mangle]
pub extern "C" fn spotka_create_identity(phone_number_c_str: *const c_char) -> *mut c_void {
    if phone_number_c_str.is_null() {
        return std::ptr::null_mut();
    }

    let phone_number = unsafe {
        match CString::from_raw(phone_number_c_str as *mut i8).into_string() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    // Generate Identity (Ed25519 keys + Phone Hash)
    let identity = crate::crypto::identity::Identity::generate(&phone_number);
    
    // Serialize the public key and phone_hash to return to the UI/Native layer
    // In a real implementation, this would be stored in Secure Enclave/KeyStore via FFI
    let blob = bincode::serialize(&identity.verifying_key.as_bytes()).unwrap_or_default();
    
    // Box the result to pass ownership to the caller (who must free it)
    Box::into_raw(Box::new(blob)) as *mut c_void
}

// --- Dictionary & Tag Parsing (CTS) ---

/// Parses a Compact Tag Sequence (CTS) string (e.g., "kino0alkohol1granie").
/// Returns a JSON string of the parsed tags on success, or an ERROR_KEY on failure.
/// 
/// IMPORTANT: The error message is a KEY (e.g., "ERR_TOO_MANY_TAGS"), not a human-readable string.
/// The UI layer is responsible for translating this key using the active language JSON file.
#[no_mangle]
pub extern "C" fn spotka_parse_tags(input_c_str: *const c_char) -> *const c_char {
    if input_c_str.is_null() {
        return CString::new("ERR_NULL_INPUT").unwrap().into_raw();
    }

    let input = unsafe {
        match CString::from_raw(input_c_str as *mut i8).into_string() {
            Ok(s) => s,
            Err(_) => return CString::new("ERR_INVALID_UTF8").unwrap().into_raw(),
        }
    };

    match crate::dict::cts_parser::parse_cts(&input) {
        Ok(tags) => {
            // Serialize successful result to JSON
            let json = serde_json::to_string(&tags).unwrap_or_else(|_| "{}".to_string());
            CString::new(json).unwrap().into_raw()
        }
        Err(error_key) => {
            // Return the ERROR KEY directly. DO NOT TRANSLATE HERE.
            // Example keys: "ERR_EXACTLY_ONE_POSITIVE_TAG_REQUIRED", "ERR_SPACE_IN_TAG"
            CString::new(error_key).unwrap().into_raw()
        }
    }
}

// --- Memory Management ---

/// Frees a C-string allocated by the Rust library.
/// Must be called by the native platform after using any string returned by Rust.
#[no_mangle]
pub extern "C" fn spotka_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Frees a generic blob allocated by the Rust library.
#[no_mangle]
pub extern "C" fn spotka_free_blob(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr as *mut Vec<u8>);
        }
    }
}

// --- Placeholder for future FFI exports ---
// These will be implemented in subsequent steps as the logic modules are fleshed out.
// Examples:
// - spotka_create_meeting(...)
// - spotka_get_nearby_meetings(...)
// - spotka_verify_trust(...)
// - spotka_start_p2p_node(...)
