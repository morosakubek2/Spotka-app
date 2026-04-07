// mobile/rust-core/src/lib.rs
// Spotka Core Library - Main Entry Point for FFI (Android/iOS).
// Architecture: 100% Rust, Anti-Social, Zero-Knowledge, Language-Agnostic.
// Year: 2026 | Rust Edition: 2024

// --- Module Declarations ---
pub mod chain;
pub mod crypto;
pub mod db;
pub mod dict;
pub mod p2p;
pub mod ui;
pub mod ffi;
pub mod sync;
pub mod app_controller;

// --- External Crates ---
use log::{info, error};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::OnceLock;
use zeroize::Zeroize; // For secure memory wiping

// --- Re-exports for easier access in FFI and UI ---
pub use app_controller::AppController;
pub use db::manager::DbManager;
pub use crypto::identity::Identity;
pub use dict::loader::GlobalDictManager;
pub use p2p::mod::P2PNode;

// --- Global State ---
// Holds the single instance of the Application Controller.
// Using OnceLock for lazy, thread-safe initialization without unsafe static mut.
static APP_CONTROLLER: OnceLock<AppController> = OnceLock::new();

/// Initializes the Spotka Core.
/// Must be called once at application startup from the Native Layer (Java/Swift).
/// 
/// # Arguments
/// * `db_path_c`: C-string path to the encrypted database file.
/// * `auth_secret_c`: C-string secret derived from biometrics/device ID (used for SQLCipher key).
/// * `device_lang_c`: C-string language code (e.g., "pl", "en") to load initial dictionary.
/// 
/// # Returns
/// * Pointer to a C-string: "OK" on success, or an error key (e.g., "ERR_DB_INIT_FAILED") on failure.
///   Caller is responsible for freeing this string using `spotka_free_string`.
#[no_mangle]
pub extern "C" fn spotka_init(
    db_path_c: *const c_char,
    auth_secret_c: *const c_char,
    device_lang_c: *const c_char,
) -> *mut c_char {
    // 1. Initialize Logger (Platform specific logic would go here, defaulting to env_logger)
    // In production, Android uses android_logger, iOS uses os_log via FFI.
    let _ = env_logger::try_init();
    info!("MSG_CORE_INIT_STARTED");

    // 2. Safe Conversion of C Strings to Rust Strings
    let db_path = match unsafe { CStr::from_ptr(db_path_c).to_str() } {
        Ok(s) => s.to_string(),
        Err(_) => return CString::new("ERR_INVALID_DB_PATH").unwrap().into_raw(),
    };

    let auth_secret = match unsafe { CStr::from_ptr(auth_secret_c).to_str() } {
        Ok(s) => s.to_string(),
        Err(_) => return CString::new("ERR_INVALID_AUTH_SECRET").unwrap().into_raw(),
    };

    let lang = match unsafe { CStr::from_ptr(device_lang_c).to_str() } {
        Ok(s) => s.to_string(),
        Err(_) => "en".to_string(), // Fallback to English
    };

    // 3. Secure Memory Handling Example (Wiping secrets after use conceptually)
    // Note: Real wiping happens inside DbManager/Identity structs using zeroize trait.
    let mut secret_buffer = auth_secret.clone();
    
    // 4. Initialize Subsystems
    // A. Database
    let db_manager = match DbManager::new(&db_path, &auth_secret) {
        Ok(db) => db,
        Err(e) => {
            error!("ERR_DB_INIT_FAILED: {}", e);
            return CString::new(e).unwrap().into_raw();
        }
    };

    // B. Dictionary (Load official dicts first)
    let dict_manager = GlobalDictManager::new();
    if let Err(e) = dict_manager.load_official_dicts(&lang) {
        error!("ERR_DICT_LOAD_FAILED: {}", e);
        // Non-fatal, but logged. App can continue with fallback.
    }

    // C. Identity (Load or Generate based on phone hash stored in secure storage)
    // For now, we assume identity is loaded from SecureStorage via FFI before this call,
    // or generated here as a placeholder.
    let identity = Identity::generate("placeholder_phone_hash"); 

    // 5. Create App Controller
    let controller = AppController::new(identity, db_manager, dict_manager);

    // 6. Store in Global State
    if APP_CONTROLLER.set(controller).is_err() {
        error!("ERR_CORE_ALREADY_INITIALIZED");
        return CString::new("ERR_CORE_ALREADY_INITIALIZED").unwrap().into_raw();
    }

    info!("MSG_CORE_INIT_SUCCESS");
    
    // Wipe sensitive buffer
    secret_buffer.zeroize();

    // Return Success
    CString::new("OK").unwrap().into_raw()
}

/// Returns the current version of the Spotka Core.
/// Useful for native layers to check compatibility.
#[no_mangle]
pub extern "C" fn spotka_get_version() -> *mut c_char {
    CString::new("0.1.0-alpha").unwrap().into_raw()
}

/// Helper function to free strings allocated by Rust and returned to C/Java/Swift.
/// Prevents memory leaks in the native layer.
#[no_mangle]
pub extern "C" fn spotka_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Example FFI function: Parse CTS tags from Native UI.
/// Returns JSON string of tags or an error key.
#[no_mangle]
pub extern "C" fn spotka_parse_cts(input_c: *const c_char) -> *mut c_char {
    let input = match unsafe { CStr::from_ptr(input_c).to_str() } {
        Ok(s) => s,
        Err(_) => return CString::new("ERR_INVALID_INPUT_STRING").unwrap().into_raw(),
    };

    match dict::cts_parser::parse_cts(input) {
        Ok(tags) => {
            // Serialize to JSON for easy consumption by UI
            match serde_json::to_string(&tags) {
                Ok(json) => CString::new(json).unwrap().into_raw(),
                Err(_) => CString::new("ERR_SERIALIZE_FAILED").unwrap().into_raw(),
            }
        }
        Err(e) => {
            // Return the error KEY directly
            CString::new(e.to_string()).unwrap().into_raw()
        }
    }
}

/// Triggers the main P2P loop (async).
/// In a real app, this would spawn a task on the runtime managed by AppController.
#[no_mangle]
pub extern "C" fn spotka_start_p2p() -> *mut c_char {
    if let Some(controller) = APP_CONTROLLER.get() {
        // Placeholder: In reality, this sends a message to the Tokio runtime channel
        // controller.start_p2p_async(); 
        info!("MSG_P2P_START_REQUESTED");
        CString::new("OK").unwrap().into_raw()
    } else {
        CString::new("ERR_CORE_NOT_INITIALIZED").unwrap().into_raw()
    }
}

// --- Unit Tests ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_string() {
        let ptr = spotka_get_version();
        let s = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert_eq!(s, "0.1.0-alpha");
        spotka_free_string(ptr);
    }
}
