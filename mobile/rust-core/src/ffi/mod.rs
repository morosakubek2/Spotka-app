// mobile/rust-core/src/ffi/mod.rs
// FFI Module: Native Bridges for Android (JNI) and iOS (Objective-C/Swift).
// Architecture: Zero-cost abstractions, safe memory handling, Language Agnostic errors.
// Year: 2026 | Rust Edition: 2024

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_os = "ios")]
pub mod ios;

// Re-export common types if needed across platforms
// pub use crate::crypto::identity::IdentityError; 
// pub use crate::db::manager::DbError;

/// Helper function to convert a Rust &str to a C-string pointer.
/// WARNING: The caller is responsible for freeing the memory (via libc::free) to avoid leaks.
/// This is critical for JNI and Objective-C interoperability.
#[no_mangle]
pub extern "C" fn ffi_string_new(s: *const libc::c_char) -> *mut libc::c_char {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    
    unsafe {
        let c_str = std::ffi::CStr::from_ptr(s);
        match c_str.to_str() {
            Ok(rust_str) => {
                // Allocate new C string on heap
                let c_string = std::ffi::CString::new(rust_str).unwrap();
                c_string.into_raw()
            },
            Err(_) => std::ptr::null_mut(),
        }
    }
}

/// Helper function to free a C-string allocated by Rust.
/// Must be called from the native side after using the string.
#[no_mangle]
pub extern "C" fn ffi_string_free(s: *mut libc::c_char) {
    if !s.is_null() {
        unsafe {
            let _ = std::ffi::CString::from_raw(s);
            // Memory is dropped here automatically
        }
    }
}

/// Global initialization hook for FFI layer.
/// Called once when the library is loaded by the OS.
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
        // iOS logging is handled via os_log in the ios module, 
        // but we can set up a fallback here if needed.
    }

    log::info!("MSG_FFI_LAYER_INITIALIZED");
}

// Optional: Export a unified error code enum for cross-platform consistency
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
}

impl From<&str> for FfiErrorCode {
    fn from(err_key: &str) -> Self {
        match err_key {
            "ERR_DB_LOCKED" => FfiErrorCode::DbLocked,
            "ERR_CRYPTO_FAILED" => FfiErrorCode::CryptoFailed,
            "ERR_P2P_DISCONNECTED" => FfiErrorCode::P2PDisconnected,
            _ => FfiErrorCode::GenericError,
        }
    }
}
