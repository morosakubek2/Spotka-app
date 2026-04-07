// mobile/rust-core/src/crypto/mod.rs
// Cryptography Module: Identity, E2EE, and Secure Storage.
// Exports: All crypto primitives needed by the app.
// Year: 2026 | Rust Edition: 2024

pub mod identity;
pub mod e2ee;
pub mod secure_storage;

// Re-export main types for cleaner imports
pub use identity::{Identity, IdentityError};
pub use e2ee::{E2EEncryptor, CryptoBox, EncryptionError};
pub use secure_storage::{SecureStorage, StorageError, BiometricAuth};

/// Cryptographic constants.
pub mod consts {
    /// Size of Ed25519 public key in bytes.
    pub const ED25519_PUBLIC_KEY_SIZE: usize = 32;
    
    /// Size of Ed25519 signature in bytes.
    pub const ED25519_SIGNATURE_SIZE: usize = 64;
    
    /// Size of X25519 shared secret in bytes.
    pub const X25519_SHARED_SECRET_SIZE: usize = 32;
    
    /// Nonce size for AES-GCM (12 bytes).
    pub const AES_GCM_NONCE_SIZE: usize = 12;
}

/// Helper to generate a random nonce (critical for AES-GCM security).
pub fn generate_nonce() -> [u8; consts::AES_GCM_NONCE_SIZE] {
    use rand::RngCore;
    let mut nonce = [0u8; consts::AES_GCM_NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce);
    nonce
}
