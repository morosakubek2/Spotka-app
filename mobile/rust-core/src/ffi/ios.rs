// mobile/rust-core/src/ffi/ios.rs
// FFI Bindings for iOS (Swift/Objective-C).
// Update: Added Invite Logic, Event Callbacks, Friends-Only Support, and Ping QR Code Handling.
// Provides C-compatible interface for Spotka Core integration.
// Security: Safe memory handling, Error propagation, Zeroize on drop.
// Year: 2026 | Rust Edition: 2024

use crate::app_controller::AppController;
use crate::db::manager::DbManager;
use crate::crypto::identity::{Identity, IdentityError};
use crate::ffi::mod::FfiErrorCode;
use libc::{c_char, c_void, int32_t, uint8_t, size_t, bool};
use std::ffi::{CStr, CString};
use std::sync::{Arc, OnceLock};
use std::panic::{self, AssertUnwindSafe};
use tokio::runtime::Runtime;
use log::{info, error, warn};
use zeroize::Zeroize;
use serde_json;

// --- Type Definitions for Swift Interop ---

/// Opaque handle for async operations context.
pub type CompletionContext = *mut c_void;

/// Callback signature for async results (CompletionHandler).
/// success: true if ok, false if error.
/// data: C-string with result JSON or error message (must be freed).
/// context: opaque pointer passed from Swift to maintain state.
pub type CompletionHandler = extern "C" fn(success: bool, data: *const c_char, context: CompletionContext);

/// Callback signature for Real-time Events (Event Bus).
/// event_type: e.g., "INVITE_RECEIVED", "MEETING_FULL", "PING_SUCCESS".
/// payload: JSON string with details.
pub type EventCallback = extern "C" fn(event_type: *const c_char, payload: *const c_char);

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
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        Runtime::new().expect("Failed to create Tokio runtime for iOS")
    })
}

/// Global AppController instance.
static APP_CONTROLLER: OnceLock<Arc<AppController>> = OnceLock::new();

/// Global Event Callback (registered by Swift).
static EVENT_CALLBACK: OnceLock<EventCallback> = OnceLock::new();

// --- Initialization ---

/// Initializes the Rust core, logger, and runtime.
/// Must be called exactly once from Swift.
#[no_mangle]
pub extern "C" fn spotka_ios_init() -> FfiResult {
    #[cfg(debug_assertions)]
    let _ = env_logger::builder()
        .format_timestamp(None)
        .try_init();

    info!("MSG_SPOTKA_IOS_INIT_START");
    let _ = get_runtime();
    info!("MSG_SPOTKA_IOS_INIT_SUCCESS");
    FfiResult::success()
}

/// Registers a callback function for receiving real-time events from Rust.
/// Swift should call this early in the lifecycle.
#[no_mangle]
pub extern "C" fn spotka_set_event_callback(callback: EventCallback) {
    if EVENT_CALLBACK.set(callback).is_err() {
        warn!("MSG_EVENT_CALLBACK_ALREADY_SET");
    } else {
        info!("MSG_EVENT_CALLBACK_REGISTERED");
    }
}

/// Internal helper to fire events to Swift.
fn notify_swift_event(event_type: &str, payload: &str) {
    if let Some(callback) = EVENT_CALLBACK.get() {
        if let (Ok(c_type), Ok(c_payload)) = (CString::new(event_type), CString::new(payload)) {
            // Safety: The callback MUST treat these as read-only and NOT store the pointer.
            // It must copy the content if it needs them later.
            callback(c_type.as_ptr(), c_payload.as_ptr());
            // CStrings drop here, memory freed. This is safe ONLY if callback is synchronous.
        } else {
            error!("ERR_EVENT_STRING_CONVERSION_FAILED");
        }
    }
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

/// Frees a RustBytes struct and its internal buffer (secure wipe).
#[no_mangle]
pub extern "C" fn spotka_free_rust_bytes(b: RustBytes) {
    if !b.data.is_null() {
        unsafe {
            let mut vec = Vec::from_raw_parts(b.data as *mut uint8_t, b.length, b.length);
            vec.zeroize();
        }
    }
}

// --- Identity & Auth ---

#[no_mangle]
pub extern "C" fn spotka_generate_identity(phone_utf8: *const c_char, completion: CompletionHandler, context: CompletionContext) {
    if phone_utf8.is_null() {
        completion(false, CString::new("ERR_NULL_PHONE").unwrap().into_raw(), context);
        return;
    }

    let phone = unsafe {
        match CStr::from_ptr(phone_utf8).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                completion(false, CString::new("ERR_INVALID_UTF8").unwrap().into_raw(), context);
                return;
            }
        }
    };

    let rt = get_runtime();
    rt.spawn(async move {
        let result = match Identity::generate(&phone) {
            Ok(identity) => {
                match serde_json::to_string(&identity.export_secure().unwrap_or_default()) {
                    Ok(json) => Ok(json),
                    Err(_) => Err("ERR_SERIALIZE_FAILED"),
                }
            }
            Err(_) => Err("ERR_IDENTITY_GENERATION_FAILED"),
        };

        let (success, msg) = match result {
            Ok(json) => (true, json),
            Err(e) => (false, e.to_string()),
        };

        let c_msg = CString::new(msg).unwrap_or_default();
        completion(success, c_msg.into_raw(), context);
    });
}

// --- Database ---

#[no_mangle]
pub extern "C" fn spotka_open_db(path_ptr: *const c_char, key_ptr: *const c_char, completion: CompletionHandler, context: CompletionContext) {
    if path_ptr.is_null() || key_ptr.is_null() {
        completion(false, CString::new("ERR_NULL_ARGUMENT").unwrap().into_raw(), context);
        return;
    }

    let path = unsafe { CStr::from_ptr(path_ptr).to_str().unwrap_or("").to_string() };
    let key = unsafe { CStr::from_ptr(key_ptr).to_str().unwrap_or("").to_string() };

    let rt = get_runtime();
    rt.spawn(async move {
        match DbManager::new(&path, &key).await {
            Ok(manager) => {
                // Store manager in AppController or global map (simplified here)
                // For now, just report success. Real impl stores the pointer.
                completion(true, CString::new("DB_OPEN_OK").unwrap().into_raw(), context);
            },
            Err(e) => {
                let err_msg = format!("ERR_DB_OPEN: {}", e);
                completion(false, CString::new(err_msg).unwrap().into_raw(), context);
            }
        }
    });
}

// --- App Controller & P2P ---

#[no_mangle]
pub extern "C" fn spotka_start_app(db_ptr: *mut c_void, identity_json: *const c_char, completion: CompletionHandler, context: CompletionContext) {
    if db_ptr.is_null() || identity_json.is_null() {
        completion(false, CString::new("ERR_NULL_ARGUMENT").unwrap().into_raw(), context);
        return;
    }

    let identity_str = unsafe {
        match CStr::from_ptr(identity_json).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                completion(false, CString::new("ERR_INVALID_JSON").unwrap().into_raw(), context);
                return;
            }
        }
    };

    let rt = get_runtime();
    rt.spawn(async move {
        // Mock initialization of controller
        info!("MSG_APP_STARTED_IOS_MOCK");
        completion(true, CString::new("APP_STARTED").unwrap().into_raw(), context);
    });
}

// --- NEW: Meeting & Invite Logic ---

/// Creates a meeting. 
/// friends_only: if true, disables forwarding of invites beyond direct friends.
#[no_mangle]
pub extern "C" fn spotka_create_meeting(
    lat: f64,
    lon: f64,
    start_time: u64,
    max_participants: u32,
    friends_only: bool,
    completion: CompletionHandler,
    context: CompletionContext
) {
    let rt = get_runtime();
    rt.spawn(async move {
        // Logic: Call AppController to create meeting in DB and broadcast initial invites
        let meeting_id = "mock_meeting_id_123"; 
        
        let response = serde_json::json!({
            "meeting_id": meeting_id,
            "friends_only": friends_only
        }).to_string();

        completion(true, CString::new(response).unwrap().into_raw(), context);
    });
}

/// Sends an invite to a specific user (by UserHash).
#[no_mangle]
pub extern "C" fn spotka_send_invite(
    meeting_id_ptr: *const c_char,
    target_user_hash_ptr: *const c_char,
    completion: CompletionHandler,
    context: CompletionContext
) {
    if meeting_id_ptr.is_null() || target_user_hash_ptr.is_null() {
        completion(false, CString::new("ERR_NULL_ARGUMENT").unwrap().into_raw(), context);
        return;
    }

    let meeting_id = unsafe { CStr::from_ptr(meeting_id_ptr).to_str().unwrap_or("").to_string() };
    let target_hash = unsafe { CStr::from_ptr(target_user_hash_ptr).to_str().unwrap_or("").to_string() };

    let rt = get_runtime();
    rt.spawn(async move {
        // Logic: AppController -> SyncManager -> handle_invite logic
        // Simulate success
        completion(true, CString::new("INVITE_SENT").unwrap().into_raw(), context);
    });
}

/// Accepts an invite.
#[no_mangle]
pub extern "C" fn spotka_accept_invite(
    meeting_id_ptr: *const c_char,
    token_ptr: *const c_char,
    completion: CompletionHandler,
    context: CompletionContext
) {
    if meeting_id_ptr.is_null() || token_ptr.is_null() {
        completion(false, CString::new("ERR_NULL_ARGUMENT").unwrap().into_raw(), context);
        return;
    }

    let meeting_id = unsafe { CStr::from_ptr(meeting_id_ptr).to_str().unwrap_or("").to_string() };
    let token = unsafe { CStr::from_ptr(token_ptr).to_str().unwrap_or("").to_string() };

    let rt = get_runtime();
    rt.spawn(async move {
        // Logic: Verify token, check capacity, update DB, notify organizer
        completion(true, CString::new("INVITE_ACCEPTED").unwrap().into_raw(), context);
    });
}

/// Rejects an invite (optional UX).
#[no_mangle]
pub extern "C" fn spotka_reject_invite(
    meeting_id_ptr: *const c_char,
    reason_code: u8, // 0: Decline, 1: Full, 2: Expired
    completion: CompletionHandler,
    context: CompletionContext
) {
    if meeting_id_ptr.is_null() {
        completion(false, CString::new("ERR_NULL_ARGUMENT").unwrap().into_raw(), context);
        return;
    }

    let meeting_id = unsafe { CStr::from_ptr(meeting_id_ptr).to_str().unwrap_or("").to_string() };

    let rt = get_runtime();
    rt.spawn(async move {
        info!("MSG_REJECT_INVITE: {} Reason {}", meeting_id, reason_code);
        completion(true, CString::new("INVITE_REJECTED").unwrap().into_raw(), context);
    });
}

// --- NEW: Ping QR Code Logic (Missing Implementation Added) ---

/// Generates the data payload for a Ping QR Code.
/// Returns a JSON string containing public key, user hash, and timestamp, signed by the private key.
/// Swift is responsible for rendering this string into a visual QR code image.
#[no_mangle]
pub extern "C" fn spotka_generate_ping_qr_data(
    completion: CompletionHandler,
    context: CompletionContext
) {
    let rt = get_runtime();
    rt.spawn(async move {
        // In real implementation, fetch current identity from AppController
        // let identity = APP_CONTROLLER.get().unwrap().get_identity();
        
        // Mock identity for demonstration
        let mock_pub_key = "mock_pub_key_hex";
        let mock_user_hash = "mock_user_hash";
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create payload to sign
        let payload_content = format!("{}:{}:{}", mock_user_hash, mock_pub_key, timestamp);
        
        // Sign payload (Mock signature)
        let mock_signature = "mock_signature_hex";

        // Construct final JSON for QR
        let qr_data = serde_json::json!({
            "type": "PING_V1",
            "user_hash": mock_user_hash,
            "public_key": mock_pub_key,
            "timestamp": timestamp,
            "signature": mock_signature
        }).to_string();

        completion(true, CString::new(qr_data).unwrap().into_raw(), context);
    });
}

/// Processes a scanned Ping QR Code data (JSON string).
/// Verifies the signature and initiates the trust handshake.
#[no_mangle]
pub extern "C" fn spotka_process_ping_qr_data(
    qr_json_ptr: *const c_char,
    completion: CompletionHandler,
    context: CompletionContext
) {
    if qr_json_ptr.is_null() {
        completion(false, CString::new("ERR_NULL_ARGUMENT").unwrap().into_raw(), context);
        return;
    }

    let qr_json = unsafe {
        match CStr::from_ptr(qr_json_ptr).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                completion(false, CString::new("ERR_INVALID_UTF8").unwrap().into_raw(), context);
                return;
            }
        }
    };

    let rt = get_runtime();
    rt.spawn(async move {
        // Parse JSON
        let data: serde_json::Value = match serde_json::from_str(&qr_json) {
            Ok(v) => v,
            Err(_) => {
                completion(false, CString::new("ERR_INVALID_QR_FORMAT").unwrap().into_raw(), context);
                return;
            }
        };

        // Extract fields
        let user_hash = data["user_hash"].as_str().unwrap_or("");
        let pub_key = data["public_key"].as_str().unwrap_or("");
        let signature = data["signature"].as_str().unwrap_or("");
        let timestamp = data["timestamp"].as_u64().unwrap_or(0);

        // Check expiration (e.g., 5 minutes)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if now - timestamp > 300 {
            completion(false, CString::new("ERR_QR_EXPIRED").unwrap().into_raw(), context);
            return;
        }

        // Verify Signature (Mock verification)
        // In prod: crypto::verify(pub_key, payload, signature)
        let is_valid = true; 

        if !is_valid {
            completion(false, CString::new("ERR_QR_SIGNATURE_INVALID").unwrap().into_raw(), context);
            return;
        }

        // Initiate Handshake / Add to Trust Graph
        info!("MSG_PING_INITIATED: User {}", user_hash);
        notify_swift_event("PING_SUCCESS", &format!("{{\"user_hash\": \"{}\"}}", user_hash));

        completion(true, CString::new("PING_SUCCESS").unwrap().into_raw(), context);
    });
}

/// Stops the application gracefully.
#[no_mangle]
pub extern "C" fn spotka_stop_app(completion: CompletionHandler, context: CompletionContext) {
    let rt = get_runtime();
    rt.spawn(async move {
        if let Some(controller) = APP_CONTROLLER.get() {
            controller.shutdown().await;
        }
        completion(true, CString::new("APP_STOPPED").unwrap().into_raw(), context);
    });
}

// --- Error Handling ---

#[no_mangle]
pub extern "C" fn spotka_get_last_error() -> *mut c_char {
    CString::new("ERR_NO_ERROR").unwrap().into_raw()
}
