// mobile/rust-core/src/crypto/mod.rs
// Cryptography Module Aggregator: Identity, E2EE, and Secure Storage.
// Architecture: Zero-Knowledge, Memory Safe, Hybrid Encryption.
// Year: 2026 | Rust Edition: 2024

pub mod identity;
pub mod e2ee;
pub mod secure_storage;

// Re-export main types for cleaner imports in other modules (e.g., app_controller, p2p, ffi)
pub use identity::{Identity, IdentityBackup, IdentityError};
pub use e2ee::{E2EE, EncryptedPacket, DecryptedPayload};
pub use secure_storage::SecureStorage;

/// Cryptographic constants to avoid magic numbers across the codebase.
pub mod consts {
    /// Size of Ed25519 public key in bytes.
    pub const ED25519_PUBLIC_KEY_SIZE: usize = 32;
    
    /// Size of Ed25519 signature in bytes.
    pub const ED25519_SIGNATURE_SIZE: usize = 64;
    
    /// Size of Ed25519 private key seed in bytes.
    pub const ED25519_SEED_SIZE: usize = 32;

    /// Size of X25519 shared secret in bytes.
    pub const X25519_SHARED_SECRET_SIZE: usize = 32;
    
    /// Nonce size for AES-GCM (12 bytes).
    pub const AES_GCM_NONCE_SIZE: usize = 12;

    /// Key size for AES-256.
    pub const AES_256_KEY_SIZE: usize = 32;
}

/// Helper to generate a cryptographically secure random nonce for AES-GCM.
/// Critical for security: Nonce must never be reused with the same key.
pub fn generate_nonce() -> [u8; consts::AES_GCM_NONCE_SIZE] {
    use rand::RngCore;
    let mut nonce = [0u8; consts::AES_GCM_NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce);
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;
    use identity::Identity;
    use secure_storage::SecureStorage; // Note: Tests for SecureStorage require mocked FFI

    #[test]
    fn test_nonce_generation_uniqueness() {
        let n1 = generate_nonce();
        let n2 = generate_nonce();
        assert_ne!(n1, n2, "Nonces must be unique");
    }

    #[test]
    fn test_constants_consistency() {
        assert_eq!(consts::AES_GCM_NONCE_SIZE, 12);
        assert_eq!(consts::ED25519_SIGNATURE_SIZE, 64);
    }

    #[test]
    fn test_identity_flow() {
        let phone = "+48999888777";
        let id = Identity::generate(phone).unwrap();
        
        // Test signing
        let msg = b"test message";
        let sig = id.sign(msg);
        
        // Test verification
        assert!(Identity::verify(&id.verifying_key, msg, &sig).is_ok());
    }
}
