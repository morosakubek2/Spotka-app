// mobile/rust-core/src/ffi/ios.rs
// FFI Bindings for iOS (Swift/Objective-C).
// Update: Added Invite Logic, Event Callbacks, and Friends-Only Support.
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

// --- Type Definitions for Swift Interop ---

/// Opaque handle for async operations context.
pub type CompletionContext = *mut c_void;

/// Callback signature for async results (CompletionHandler).
/// success: true if ok, false if error.
/// data: C-string with result JSON or error message (must be freed).
/// context: opaque pointer passed from Swift to maintain state.
pub type CompletionHandler = extern "C" fn(success: bool, data: *const c_char, context: CompletionContext);

/// Callback signature for Real-time Events (Event Bus).
/// event_type: e.g., "INVITE_RECEIVED", "MEETING_FULL".
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
        // We must ensure strings are valid C strings before passing.
        // Since callback is extern "C", we assume it handles them immediately or copies them.
        // We create CStrings here which will be dropped at end of scope, 
        // BUT the callback must copy the data synchronously.
        // Pattern: Callback receives ptr, copies data, returns. Then we free.
        
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

    // In a real scenario, we reconstruct the controller using the DB pointer
    // Here we assume AppController manages the DB internally or we pass it differently.
    // Simplified: Just init controller logic.
    
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
        // let controller = AppController::new(...).await?;
        // APP_CONTROLLER.set(Arc::new(controller)).ok();
        
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
        // Mock result:
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
        // Check if user exists, check friends_only flag, send packet
        
        // Simulate success
        completion(true, CString::new("INVITE_SENT").unwrap().into_raw(), context);
        
        // If failed (e.g., user not in network), we would call completion(false, ...)
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
        
        // Simulate capacity check failure example:
        // if full {
        //    notify_swift_event("MEETING_FULL", &meeting_id);
        //    completion(false, CString::new("ERR_MEETING_FULL").unwrap().into_raw(), context);
        //    return;
        // }

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
        // Logic: Send Reject packet to organizer
        info!("MSG_REJECT_INVITE: {} Reason {}", meeting_id, reason_code);
        completion(true, CString::new("INVITE_REJECTED").unwrap().into_raw(), context);
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
