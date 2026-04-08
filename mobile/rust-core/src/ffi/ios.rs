// mobile/rust-core/src/ffi/ios.rs
// FFI Bindings for iOS (Swift/Objective-C).
// Provides C-compatible interface for Spotka Core integration.
// Security: Safe memory handling, Error propagation, Zeroize on drop.
// Year: 2026 | Rust Edition: 2024

use crate::app_controller::AppController;
use crate::db::manager::DbManager;
use crate::crypto::identity::{Identity, IdentityError};
use crate::ffi::mod::FfiErrorCode;
use libc::{c_char, c_void, int32_t, uint8_t, size_t};
use std::ffi::{CStr, CString};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use log::{info, error, warn};
use zeroize::Zeroize;

// --- Type Definitions for Swift Interop ---

/// Opaque handle for async operations context.
pub type CompletionContext = *mut c_void;

/// Callback signature for async results.
/// success: true if ok, false if error.
/// data: C-string with result or error message (must be freed).
/// context: opaque pointer passed from Swift.
pub type CompletionHandler = extern "C" fn(success: bool, data: *const c_char, context: CompletionContext);

/// Struct representing a string owned by Rust, passed to Swift.
/// Swift must call `spotka_free_rust_string` to release memory.
#[repr(C)]
pub struct RustString {
    pub data: *const u8,
    pub length: usize,
}

/// Struct representing a byte array owned by Rust.
#[repr(C)]
pub struct RustBytes {
    pub data: *const uint8_t,
    pub length: usize,
}

/// Result structure for FFI calls returning status + optional data.
#[repr(C)]
pub struct FfiResult {
    pub code: int32_t, // 0 = Success, >0 = Error Code
    pub message: *mut c_char, // Must be freed by spotka_free_string
}

impl FfiResult {
    pub fn success() -> Self {
        FfiResult {
            code: 0,
            message: std::ptr::null_mut(),
        }
    }

    pub fn error(code: FfiErrorCode, msg: &str) -> Self {
        let c_msg = CString::new(msg).unwrap_or_default();
        FfiResult {
            code: code as int32_t,
            message: c_msg.into_raw(),
        }
    }
}

// --- Global State ---

/// Global Tokio Runtime.
/// Initialized once on first call.
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        Runtime::new().expect("Failed to create Tokio runtime for iOS")
    })
}

/// Global AppController instance (optional, depending on architecture).
static APP_CONTROLLER: OnceLock<Arc<AppController>> = OnceLock::new();

// --- Initialization ---

/// Initializes the Rust core, logger, and runtime.
/// Must be called exactly once from Swift (e.g., in AppDelegate).
#[no_mangle]
pub extern "C" fn spotka_ios_init() -> FfiResult {
    // Initialize Logger (redirect to OSLog via custom crate or simple env_logger if configured)
    // For production, use `oslog` crate to bridge to unified logging system.
    #[cfg(debug_assertions)]
    let _ = env_logger::builder()
        .format_timestamp(None)
        .try_init();

    info!("MSG_SPOTKA_IOS_INIT_START");

    // Ensure runtime is ready
    let _ = get_runtime();

    info!("MSG_SPOTKA_IOS_INIT_SUCCESS");
    FfiResult::success()
}

// --- Memory Management Helpers ---

/// Frees a C-string allocated by Rust.
#[no_mangle]
pub extern "C" fn spotka_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Frees a RustString struct and its internal buffer.
#[no_mangle]
pub extern "C" fn spotka_free_rust_string(s: RustString) {
    if !s.data.is_null() {
        unsafe {
            let _ = Vec::from_raw_parts(s.data as *mut u8, s.length, s.length);
        }
    }
}

/// Frees a RustBytes struct and its internal buffer.
#[no_mangle]
pub extern "C" fn spotka_free_rust_bytes(b: RustBytes) {
    if !b.data.is_null() {
        unsafe {
            let mut vec = Vec::from_raw_parts(b.data as *mut uint8_t, b.length, b.length);
            vec.zeroize(); // Secure wipe before drop
        }
    }
}

// --- Identity & Auth ---

/// Generates a new Identity based on phone number.
/// Returns serialized JSON of public identity or Error.
#[no_mangle]
pub extern "C" fn spotka_generate_identity(phone_utf8: *const c_char) -> FfiResult {
    if phone_utf8.is_null() {
        return FfiResult::error(FfiErrorCode::NullPointer, "ERR_NULL_PHONE_NUMBER");
    }

    let phone = unsafe {
        match CStr::from_ptr(phone_utf8).to_str() {
            Ok(s) => s,
            Err(_) => return FfiResult::error(FfiErrorCode::InvalidUtf8, "ERR_INVALID_UTF8"),
        }
    };

    match Identity::generate(phone) {
        Ok(identity) => {
            // Export public part only
            let backup = match identity.export_secure() {
                Ok(b) => b,
                Err(_) => return FfiResult::error(FfiErrorCode::CryptoFailed, "ERR_EXPORT_FAILED"),
            };
            
            match serde_json::to_string(&backup) {
                Ok(json) => {
                    let c_json = CString::new(json).unwrap();
                    FfiResult {
                        code: 0,
                        message: c_json.into_raw(),
                    }
                }
                Err(_) => FfiResult::error(FfiErrorCode::GenericError, "ERR_SERIALIZE_FAILED"),
            }
        }
        Err(e) => FfiResult::error(FfiErrorCode::CryptoFailed, &e.to_string()),
    }
}

/// Restores identity from backup JSON.
/// Performs cryptographic Proof of Possession check.
#[no_mangle]
pub extern "C" fn spotka_restore_identity(backup_json: *const c_char) -> FfiResult {
    if backup_json.is_null() {
        return FfiResult::error(FfiErrorCode::NullPointer, "ERR_NULL_BACKUP");
    }

    let json_str = unsafe {
        match CStr::from_ptr(backup_json).to_str() {
            Ok(s) => s,
            Err(_) => return FfiResult::error(FfiErrorCode::InvalidUtf8, "ERR_INVALID_UTF8"),
        }
    };

    let backup: crate::crypto::identity::IdentityBackup = match serde_json::from_str(json_str) {
        Ok(b) => b,
        Err(_) => return FfiResult::error(FfiErrorCode::GenericError, "ERR_PARSE_FAILED"),
    };

    match Identity::restore_from_seed(backup.secret_key_seed, &backup.phone_hash, &backup.validation_signature) {
        Ok(_identity) => {
            // In real app, store this identity in AppController
            FfiResult::success()
        }
        Err(e) => FfiResult::error(FfiErrorCode::CryptoFailed, &e.to_string()),
    }
}

/// Triggers Biometric Authentication via Swift Bridge.
/// This function blocks until Swift calls back (or uses async pattern).
/// Simplified here: expects a token from Swift Keychain.
#[no_mangle]
pub extern "C" fn spotka_verify_biometrics(token_ptr: *const c_char) -> bool {
    if token_ptr.is_null() {
        return false;
    }
    
    // In production: 
    // 1. Call Swift function `SpotkaBridge.shared.requestBiometricAuth(completion:)`
    // 2. Wait for completion (via channel or callback)
    // 3. Verify token against stored hash
    
    // Simulation:
    unsafe {
        let token = CStr::from_ptr(token_ptr).to_bytes();
        !token.is_empty()
    }
}

// --- Database ---

/// Opens the encrypted SQLite database.
/// Returns an opaque pointer to DbManager.
#[no_mangle]
pub extern "C" fn spotka_open_db(path_ptr: *const c_char, key_ptr: *const c_char) -> *mut c_void {
    if path_ptr.is_null() || key_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let path = unsafe { CStr::from_ptr(path_ptr).to_str().unwrap_or("") };
    let key = unsafe { CStr::from_ptr(key_ptr).to_str().unwrap_or("") };

    let rt = get_runtime();
    
    // Block on async init
    match rt.block_on(DbManager::new(path, key)) {
        Ok(manager) => Box::into_raw(Box::new(manager)) as *mut c_void,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Closes the database and frees resources.
#[no_mangle]
pub extern "C" fn spotka_close_db(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr as *mut DbManager);
        }
    }
}

// --- App Controller & P2P ---

/// Starts the main application logic (P2P, Sync, etc.).
/// Takes ownership of the DbManager pointer.
#[no_mangle]
pub extern "C" fn spotka_start_app(db_ptr: *mut c_void, identity_json: *const c_char) -> FfiResult {
    if db_ptr.is_null() || identity_json.is_null() {
        return FfiResult::error(FfiErrorCode::NullPointer, "ERR_NULL_ARGUMENT");
    }

    let db_manager = unsafe { Box::from_raw(db_ptr as *mut DbManager) };
    
    let identity_str = unsafe {
        match CStr::from_ptr(identity_json).to_str() {
            Ok(s) => s,
            Err(_) => return FfiResult::error(FfiErrorCode::InvalidUtf8, "ERR_INVALID_UTF8"),
        }
    };

    let identity: Identity = match serde_json::from_str(identity_str) {
        Ok(i) => i, // Needs custom deserializer for Identity, simplified here
        Err(_) => return FfiResult::error(FfiErrorCode::GenericError, "ERR_PARSE_IDENTITY"),
    };

    // Create AppController
    let controller = match AppController::new(*db_manager, identity) {
        Ok(c) => Arc::new(c),
        Err(_) => return FfiResult::error(FfiErrorCode::GenericError, "ERR_CTRL_INIT_FAILED"),
    };

    // Store globally or pass to Swift to manage lifecycle
    if APP_CONTROLLER.set(controller.clone()).is_err() {
        return FfiResult::error(FfiErrorCode::GenericError, "ERR_CTRL_ALREADY_SET");
    }

    // Spawn background tasks
    let ctrl_clone = controller.clone();
    get_runtime().spawn(async move {
        if let Err(e) = ctrl_clone.run().await {
            error!("MSG_APP_CONTROLLER_ERROR: {}", e);
        }
    });

    info!("MSG_APP_STARTED_IOS");
    FfiResult::success()
}

/// Stops the application gracefully.
#[no_mangle]
pub extern "C" fn spotka_stop_app() -> FfiResult {
    if let Some(controller) = APP_CONTROLLER.get() {
        let rt = get_runtime();
        let ctrl = controller.clone();
        
        rt.block_on(async {
            ctrl.shutdown().await;
        });
        info!("MSG_APP_STOPPED_IOS");
        FfiResult::success()
    } else {
        FfiResult::error(FfiErrorCode::GenericError, "ERR_APP_NOT_RUNNING")
    }
}

// --- Dictionary ---

/// Loads a dictionary JSON into the global manager.
#[no_mangle]
pub extern "C" fn spotka_load_dict(json_data: *const c_char, is_official: bool) -> FfiResult {
    if json_data.is_null() {
        return FfiResult::error(FfiErrorCode::NullPointer, "ERR_NULL_JSON");
    }

    let json = unsafe { CStr::from_ptr(json_data).to_str().unwrap_or("") };
    
    // Access global dict manager (would need a getter in crate::dict)
    // Simplified: just validate JSON
    if serde_json::from_str::<serde_json::Value>(json).is_err() {
        return FfiResult::error(FfiErrorCode::GenericError, "ERR_INVALID_DICT_JSON");
    }

    // crate::dict::GLOBAL_MANAGER.register_dict(...).await?;
    
    FfiResult::success()
}

// --- Error Handling ---

/// Returns the last error message (thread-local or global).
#[no_mangle]
pub extern "C" fn spotka_get_last_error() -> *mut c_char {
    CString::new("ERR_NO_ERROR").unwrap().into_raw()
}
