// mobile/rust-core/src/ffi/mod.rs
// FFI Module: Native Bridges for Android (JNI) and iOS (Objective-C/Swift).
// Architecture: Zero-cost abstractions, safe memory handling, Language Agnostic errors.
// Features: String helpers, Byte buffers, Global JVM context (Android), Unified Result types,
//           P2P Event Marshalling for Private Mesh (Invites, Status Updates).
// Year: 2026 | Rust Edition: 2024

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_os = "ios")]
pub mod ios;

use log::{info, error};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

// --- Global JVM Context (Android Only) ---
#[cfg(target_os = "android")]
static mut JAVA_VM: *mut jni::JavaVM = ptr::null_mut();
#[cfg(target_os = "android")]
static mut ACTIVITY_CONTEXT: *mut jni::objects::JObject = ptr::null_mut();

#[cfg(target_os = "android")]
pub fn set_java_vm(vm: *mut jni::JavaVM) {
    unsafe { JAVA_VM = vm };
}

#[cfg(target_os = "android")]
pub fn get_java_vm() -> Option<&'static jni::JavaVM> {
    unsafe { JAVA_VM.as_ref() }
}

#[cfg(target_os = "android")]
pub fn set_activity_context(ctx: *mut jni::objects::JObject) {
    unsafe { ACTIVITY_CONTEXT = ctx };
}

#[cfg(target_os = "android")]
pub fn get_activity_context() -> Option<&'static jni::objects::JObject> {
    unsafe { ACTIVITY_CONTEXT.as_ref() }
}

// --- Safe String Conversion Helpers ---

/// Converts a C-string (from native) to a Rust String.
pub fn copy_cstr_to_rust(c_str: *const c_char) -> Result<String, &'static str> {
    if c_str.is_null() {
        return Err("ERR_NULL_POINTER");
    }
    unsafe {
        CStr::from_ptr(c_str)
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_| "ERR_INVALID_UTF8")
    }
}

/// Converts a Rust &str to a newly allocated C-string.
/// CALLER (Native Side) is responsible for freeing this memory using `ffi_free_cstring`.
pub fn copy_rust_str_to_cstr(rust_str: &str) -> *mut c_char {
    match CString::new(rust_str) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Frees a C-string allocated by Rust.
#[no_mangle]
pub extern "C" fn ffi_free_cstring(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

// --- Byte Buffer Helpers ---

#[repr(C)]
pub struct FfiByteBuffer {
    pub data: *mut u8,
    pub len: usize,
}

impl FfiByteBuffer {
    pub fn from_vec(vec: Vec<u8>) -> Self {
        let mut vec = vec;
        let ptr = vec.as_mut_ptr();
        let len = vec.len();
        std::mem::forget(vec);
        FfiByteBuffer { data: ptr, len }
    }
}

#[no_mangle]
pub extern "C" fn ffi_free_byte_buffer(buffer: FfiByteBuffer) {
    if !buffer.data.is_null() && buffer.len > 0 {
        unsafe {
            let _vec = Vec::from_raw_parts(buffer.data, buffer.len, buffer.len);
        }
    }
}

// --- Unified Error Codes (Expanded for P2P Mesh) ---

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum FfiErrorCode {
    Success = 0,
    GenericError = 1,
    NullPointer = 2,
    InvalidUtf8 = 3,
    DbLocked = 4,
    CryptoFailed = 5,
    P2PDisconnected = 6,
    IdentityNotFound = 7,
    BiometricAuthFailed = 8,
    
    // QR Code Errors
    PingInvalidCode = 15,      // Kod nieczytelny lub uszkodzony
    PingSignatureFailed = 16,  // Błąd weryfikacji podpisu
    PingSelfAttempt = 17,      // Próba pingowania samego siebie
    PingAlreadyExists = 18,    // Znajomy już dodany
    
    // NEW: P2P Mesh Specific Errors
    MeetingFull = 9,
    FriendsOnlyRestricted = 10,
    NotInNetwork = 11,
    InviteExpired = 12,
    UnauthorizedAction = 13,
    RateLimitExceeded = 14,
}

impl From<&str> for FfiErrorCode {
    fn from(err_key: &str) -> Self {
        match err_key {
            "ERR_DB_LOCKED" => FfiErrorCode::DbLocked,
            "ERR_CRYPTO_FAILED" => FfiErrorCode::CryptoFailed,
            "ERR_P2P_DISCONNECTED" => FfiErrorCode::P2PDisconnected,
            "ERR_IDENTITY_NOT_FOUND" => FfiErrorCode::IdentityNotFound,
            "ERR_BIOMETRIC_AUTH_FAILED" => FfiErrorCode::BiometricAuthFailed,
            "ERR_NULL_POINTER" => FfiErrorCode::NullPointer,
            "ERR_INVALID_UTF8" => FfiErrorCode::InvalidUtf8,
            
            // New mappings
            "ERR_MEETING_FULL" => FfiErrorCode::MeetingFull,
            "ERR_FORWARDING_RESTRICTED" | "ERR_FRIENDS_ONLY" => FfiErrorCode::FriendsOnlyRestricted,
            "ERR_TARGET_NOT_IN_NETWORK" => FfiErrorCode::NotInNetwork,
            "ERR_INVITE_EXPIRED" => FfiErrorCode::InviteExpired,
            "ERR_UNAUTHORIZED_PARTICIPATION" => FfiErrorCode::UnauthorizedAction,
            "ERR_RATE_LIMIT_EXCEEDED" => FfiErrorCode::RateLimitExceeded,
            
            _ => FfiErrorCode::GenericError,
        }
    }
}

// --- P2P Event Structure (For pushing to UI) ---

/// Represents a P2P event pushed from Rust to Native UI.
/// Memory management: Native side must call `ffi_free_p2p_event` after processing.
#[repr(C)]
pub struct FfiP2PEvent {
    pub event_type: u8, // 0: InviteReceived, 1: ParticipationUpdate, 2: PeerConnected, 3: Error
    pub meeting_id: *mut c_char,
    pub user_id: *mut c_char,
    pub details: *mut c_char, // e.g., Status string or Error message
    pub error_code: FfiErrorCode,
}

impl FfiP2PEvent {
    pub fn new_invite_received(meeting_id: &str, organizer_id: &str) -> Self {
        FfiP2PEvent {
            event_type: 0,
            meeting_id: copy_rust_str_to_cstr(meeting_id),
            user_id: copy_rust_str_to_cstr(organizer_id),
            details: ptr::null_mut(),
            error_code: FfiErrorCode::Success,
        }
    }

    pub fn new_participation_update(meeting_id: &str, user_id: &str, status: &str) -> Self {
        FfiP2PEvent {
            event_type: 1,
            meeting_id: copy_rust_str_to_cstr(meeting_id),
            user_id: copy_rust_str_to_cstr(user_id),
            details: copy_rust_str_to_cstr(status),
            error_code: FfiErrorCode::Success,
        }
    }

    pub fn new_error(meeting_id: Option<&str>, msg: &str, code: FfiErrorCode) -> Self {
        FfiP2PEvent {
            event_type: 3,
            meeting_id: match meeting_id {
                Some(id) => copy_rust_str_to_cstr(id),
                None => ptr::null_mut(),
            },
            user_id: ptr::null_mut(),
            details: copy_rust_str_to_cstr(msg),
            error_code: code,
        }
    }
}

/// Frees memory associated with an FfiP2PEvent.
#[no_mangle]
pub extern "C" fn ffi_free_p2p_event(event: FfiP2PEvent) {
    if !event.meeting_id.is_null() {
        unsafe { let _ = CString::from_raw(event.meeting_id); }
    }
    if !event.user_id.is_null() {
        unsafe { let _ = CString::from_raw(event.user_id); }
    }
    if !event.details.is_null() {
        unsafe { let _ = CString::from_raw(event.details); }
    }
}

// --- Global Initialization ---

#[ctor::ctor]
fn ffi_init() {
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(log::Level::Info)
            .with_tag("SpotkaCore")
    );

    #[cfg(target_os = "ios")]
    {
        // iOS logging handled via os_log
    }

    info!("MSG_FFI_LAYER_INITIALIZED");
}

// --- Test/Debug Exports ---

#[no_mangle]
pub extern "C" fn ffi_get_version() -> *mut c_char {
    copy_rust_str_to_cstr(env!("CARGO_PKG_VERSION"))
}
