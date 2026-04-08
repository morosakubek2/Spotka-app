// mobile/rust-core/src/ffi/mod.rs
// FFI Module: Native Bridges for Android (JNI) and iOS (Objective-C/Swift).
// Architecture: Zero-cost abstractions, safe memory handling, Language Agnostic errors.
// Features: String helpers, Byte buffers, Global JVM context (Android), Unified Result types.
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
/// Caller retains ownership of the C-string (does not free it).
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
/// Must be called from the native side after using the string returned by Rust.
#[no_mangle]
pub extern "C" fn ffi_free_cstring(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
            // Memory dropped here
        }
    }
}

// --- Byte Buffer Helpers (For Crypto/P2P) ---

/// Represents a buffer of bytes allocated by Rust.
/// Native side must call `ffi_free_byte_buffer` after reading data.
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
        std::mem::forget(vec); // Prevent Rust from dropping it
        FfiByteBuffer { data: ptr, len }
    }
}

/// Frees a byte buffer allocated by Rust.
#[no_mangle]
pub extern "C" fn ffi_free_byte_buffer(buffer: FfiByteBuffer) {
    if !buffer.data.is_null() && buffer.len > 0 {
        unsafe {
            // Reconstruct vector to drop it safely
            let _vec = Vec::from_raw_parts(buffer.data, buffer.len, buffer.len);
        }
    }
}

// --- Unified Error Codes ---

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
            _ => FfiErrorCode::GenericError,
        }
    }
}

// --- Global Initialization ---

#[ctor::ctor]
fn ffi_init() {
    // Initialize logger based on platform
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(log::Level::Info)
            .with_tag("SpotkaCore")
    );

    #[cfg(target_os = "ios")]
    {
        // iOS logging handled via os_log in ios module
    }

    info!("MSG_FFI_LAYER_INITIALIZED");
}

// --- Test FFI Exports (Optional for debugging) ---

#[no_mangle]
pub extern "C" fn ffi_get_version() -> *mut c_char {
    copy_rust_str_to_cstr(env!("CARGO_PKG_VERSION"))
}
