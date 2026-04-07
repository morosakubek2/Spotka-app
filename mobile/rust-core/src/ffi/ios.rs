// mobile/rust-core/src/ffi/ios.rs
// FFI Bindings for iOS (Swift/Objective-C).
// Provides C-compatible interface for Spotka Core integration.
// Year: 2026 | Rust Edition: 2024

use crate::app_controller::AppController;
use crate::db::manager::DbManager;
use crate::crypto::identity::Identity;
use libc::{c_char, c_void, int32_t, uint8_t};
use std::ffi::{CStr, CString};
use std::os::raw::c_long;
use std::sync::Arc;
use tokio::runtime::Runtime;
use log::{info, error};

// --- Type Definitions for Swift Interop ---
#[repr(C)]
pub struct RustString {
    pub data: *const u8,
    pub length: usize,
}

#[repr(C)]
pub struct RustBytes {
    pub data: *const uint8_t,
    pub length: usize,
}

// Global Runtime for async operations
static mut RUNTIME: Option<Runtime> = None;

// Callback type for async results (Swift closure equivalent)
pub type CompletionHandler = extern "C" fn(success: bool, data: *const c_char, context: *mut c_void);

// --- Initialization ---

/// Initializes the Rust runtime and logger.
/// Must be called once at app launch from Swift.
#[no_mangle]
pub extern "C" fn spotka_ios_init() {
    unsafe {
        if RUNTIME.is_none() {
            RUNTIME = Some(Runtime::new().expect("Failed to create Tokio runtime"));
        }
    }
    // Initialize logger to output to OSLog (via custom layer or env_logger setup)
    env_logger::init();
    info!("MSG_SPOTKA_IOS_INIT_SUCCESS");
}

// --- Memory Management Helpers ---

/// Frees a C-string allocated by Rust.
/// Swift must call this after using a string returned from Rust.
#[no_mangle]
pub extern "C" fn spotka_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Frees a byte array allocated by Rust.
#[no_mangle]
pub extern "C" fn spotka_free_bytes(ptr: *mut uint8_t, len: usize) {
    if !ptr.is_null() {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }
    }
}

// --- Identity & Auth ---

/// Generates a new Identity based on phone number hash.
/// Returns a serialized public identity blob.
#[no_mangle]
pub extern "C" fn spotka_generate_identity(phone_utf8: *const c_char) -> *mut c_char {
    let phone = unsafe { CStr::from_ptr(phone_utf8).to_str().unwrap_or("") };
    let identity = Identity::generate(phone);
    
    // Serialize public part (for storage/sharing)
    let serialized = serde_json::to_string(&identity.verifying_key()).unwrap_or_default();
    CString::new(serialized).unwrap().into_raw()
}

/// Simulates Biometric Auth check (calls into Swift via callback in real impl).
/// For now, returns success if a dummy token is provided.
#[no_mangle]
pub extern "C" fn spotka_verify_biometrics(token: *const c_char) -> bool {
    // In production: invoke Swift callback to trigger FaceID/TouchID
    // and wait for result. Here we assume success for demo.
    !token.is_null()
}

// --- Database ---

/// Opens the encrypted database.
/// Returns an opaque pointer to DbManager.
#[no_mangle]
pub extern "C" fn spotka_open_db(path: *const c_char, auth_token: *const c_char) -> *mut c_void {
    let path_str = unsafe { CStr::from_ptr(path).to_str().unwrap_or("") };
    let token_str = unsafe { CStr::from_ptr(auth_token).to_str().unwrap_or("") };

    let runtime = unsafe { RUNTIME.as_ref().unwrap() };
    let manager = runtime.block_on(async {
        DbManager::new(path_str, token_str).await.ok()
    });

    match manager {
        Some(m) => Box::into_raw(Box::new(m)) as *mut c_void,
        None => std::ptr::null_mut(),
    }
}

/// Closes the database and frees memory.
#[no_mangle]
pub extern "C" fn spotka_close_db(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr as *mut DbManager);
        }
    }
}

// --- P2P Network ---

/// Starts the P2P node in background.
/// Takes the DbManager pointer and Identity.
#[no_mangle]
pub extern "C" fn spotka_start_p2p(db_ptr: *mut c_void, identity_json: *const c_char) {
    let db_manager = unsafe { *(db_ptr as *mut DbManager) };
    let identity_str = unsafe { CStr::from_ptr(identity_json).to_str().unwrap_or("") };
    let identity: Identity = serde_json::from_str(identity_str).unwrap(); // Simplified

    let runtime = unsafe { RUNTIME.as_ref().unwrap() };
    runtime.spawn(async move {
        // Initialize and run P2P node
        // let node = P2PNode::new(identity, db_manager).await?;
        // node.run().await?;
        info!("MSG_P2P_NODE_STARTED_IOS");
    });
}

// --- Dictionary & Sync ---

/// Loads a dictionary from a JSON string.
/// Returns true if successful.
#[no_mangle]
pub extern "C" fn spotka_load_dict(json_data: *const c_char) -> bool {
    let json = unsafe { CStr::from_ptr(json_data).to_str().unwrap_or("") };
    // Call dict::loader::load_from_json(json)
    // For now, just parse to check validity
    serde_json::from_str::<serde_json::Value>(json).is_ok()
}

// --- Error Handling ---

/// Returns the last error message as a C-string (must be freed).
#[no_mangle]
pub extern "C" fn spotka_get_last_error() -> *mut c_char {
    // In production, maintain a thread-local error buffer
    CString::new("ERR_NO_ERROR").unwrap().into_raw()
}
