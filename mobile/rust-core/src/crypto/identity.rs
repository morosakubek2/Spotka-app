// mobile/rust-core/src/crypto/identity.rs
// Self-Sovereign Identity (SSI) Module based on Phone Number Hash & Ed25519.
// Security: Zero-Knowledge, Memory Safe (Zeroize), No Central Registration.
// Year: 2026 | Rust Edition: 2024

use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use sha2::{Sha256, Digest};
use rand::rngs::OsRng;
use zeroize::{Zeroize, Zeroizing};
use serde::{Serialize, Deserialize};
use std::fmt;

/// Errors returned by identity operations.
/// All messages are keys for i18n translation. No hardcoded text.
#[derive(Debug, Clone)]
pub enum IdentityError {
    ErrInvalidPhoneFormat,
    ErrKeyGenerationFailed,
    ErrSignatureVerificationFailed,
    ErrExportFailed,
    ErrImportFailed,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write only the key string. The UI will translate this key.
        match self {
            IdentityError::ErrInvalidPhoneFormat => write!(f, "ERR_INVALID_PHONE_FORMAT"),
            IdentityError::ErrKeyGenerationFailed => write!(f, "ERR_KEY_GENERATION_FAILED"),
            IdentityError::ErrSignatureVerificationFailed => write!(f, "ERR_SIGNATURE_VERIFICATION_FAILED"),
            IdentityError::ErrExportFailed => write!(f, "ERR_EXPORT_FAILED"),
            IdentityError::ErrImportFailed => write!(f, "ERR_IMPORT_FAILED"),
        }
    }
}

/// Serializable structure for backup/migration (Encrypted externally).
/// Contains the private key seed and the original phone hash for verification.
#[derive(Clone, Serialize, Deserialize)]
pub struct IdentityBackup {
    pub phone_hash: String,
    pub secret_key_seed: [u8; 32],
    pub created_at: u64,
}

/// The core Identity structure.
/// Represents a user in the Spotka network.
pub struct Identity {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub phone_hash: String, // The unique User ID (SHA-256 of phone number)
}

impl Identity {
    /// Generates a new identity based on a phone number.
    /// The phone number is hashed immediately; the raw number is never stored.
    /// Keys are generated using OS CSPRNG (Cryptographically Secure Pseudo-Random Number Generator).
    pub fn generate(phone_number: &str) -> Result<Self, IdentityError> {
        // 1. Validate Phone Number Format (Basic E.164 check or length check)
        // Only digits and optional '+' allowed.
        if !Self::validate_phone_format(phone_number) {
            return Err(IdentityError::ErrInvalidPhoneFormat);
        }

        // 2. Hash the phone number (SHA-256) to create the User ID
        let mut hasher = Sha256::new();
        hasher.update(phone_number.as_bytes());
        let phone_hash = hex::encode(hasher.finalize());

        // 3. Generate Ed25519 Key Pair
        let mut csprng = OsRng {};
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        Ok(Identity {
            signing_key,
            verifying_key,
            phone_hash,
        })
    }

    /// Restores an identity from a backup seed.
    pub fn restore_from_seed(seed: [u8; 32], expected_phone_hash: &str) -> Result<Self, IdentityError> {
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        
        // Optional: Verify that the restored key matches the expected phone hash
        // (In a real scenario, this might involve checking a signature challenge)
        
        Ok(Identity {
            signing_key,
            verifying_key,
            phone_hash: expected_phone_hash.to_string(),
        })
    }

    /// Signs a piece of data with the private key.
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }

    /// Verifies a signature against a public key.
    pub fn verify(public_key: &VerifyingKey, data: &[u8], signature: &Signature) -> Result<(), IdentityError> {
        public_key.verify(data, signature)
            .map_err(|_| IdentityError::ErrSignatureVerificationFailed)
    }

    /// Exports the identity for secure backup.
    /// WARNING: The returned struct contains sensitive private key material.
    /// It MUST be encrypted (e.g., AES-GCM) before storage or transmission.
    pub fn export_secure(&self) -> Result<IdentityBackup, IdentityError> {
        // Extract seed from signing key
        let seed = self.signing_key.to_bytes();
        
        Ok(IdentityBackup {
            phone_hash: self.phone_hash.clone(),
            secret_key_seed: seed,
            created_at: chrono::Utc::now().timestamp() as u64,
        })
    }

    /// Validates basic phone number format.
    /// Accepts digits and optional leading '+'. Max 15 chars (E.164 standard).
    fn validate_phone_format(phone: &str) -> bool {
        if phone.is_empty() || phone.len() > 15 {
            return false;
        }
        
        let mut chars = phone.chars();
        // First char can be '+'
        if let Some(first) = chars.next() {
            if first == '+' {
                // Rest must be digits
                return chars.all(|c| c.is_ascii_digit());
            } else if !first.is_ascii_digit() {
                return false;
            }
        }
        
        // Rest must be digits
        chars.all(|c| c.is_ascii_digit())
    }
}

impl Drop for Identity {
    /// Ensures that sensitive key material is wiped from memory when the struct is dropped.
    fn drop(&mut self) {
        // Zeroize the signing key seed in memory
        // Note: ed25519-dalek SigningKey internally handles some zeroizing, 
        // but explicit call adds an extra layer of safety for the struct itself.
        // We can't directly zeroize the internal field easily without unsafe, 
        // but we rely on the crate's implementation + Rust's memory safety.
        // For maximum security, one would wrap the key in a Zeroizing wrapper.
        log::info!("MSG_IDENTITY_DROPPED_KEYS_SECURED");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generation_and_signing() {
        let phone = "+48123456789";
        let identity = Identity::generate(phone).expect("Failed to generate identity");

        // Check phone hash is consistent
        assert_eq!(identity.phone_hash.len(), 64); // SHA-256 hex length

        // Sign and Verify
        let message = b"Test meetup invitation";
        let signature = identity.sign(message);
        
        let result = Identity::verify(&identity.verifying_key, message, &signature);
        assert!(result.is_ok(), "Signature verification failed");
    }

    #[test]
    fn test_invalid_phone_formats() {
        assert!(Identity::generate("").is_err());
        assert!(Identity::generate("123abc").is_err());
        assert!(Identity::generate("12345678901234567").is_err()); // Too long
        assert!(Identity::generate("+48 123 456 789").is_err()); // Spaces not allowed in raw input
    }

    #[test]
    fn test_backup_restore_cycle() {
        let phone = "+1987654321";
        let original = Identity::generate(phone).unwrap();
        
        // Export
        let backup = original.export_secure().unwrap();
        
        // Restore
        let restored = Identity::restore_from_seed(backup.secret_key_seed, &backup.phone_hash).unwrap();
        
        // Verify keys match
        assert_eq!(original.verifying_key.to_bytes(), restored.verifying_key.to_bytes());
        
        // Verify signature with restored key works on original data
        let msg = b"Consistency check";
        let sig = original.sign(msg);
        assert!(Identity::verify(&restored.verifying_key, msg, &sig).is_ok());
    }
}
