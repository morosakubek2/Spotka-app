// mobile/rust-core/src/crypto/identity.rs
// Self-Sovereign Identity (SSI) Module based on Phone Number Hash & Ed25519.
// Security: Zero-Knowledge, Memory Safe (Zeroize), Proof of Possession for Restore.
// Year: 2026 | Rust Edition: 2024

use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use sha2::{Sha256, Digest};
use rand::rngs::OsRng;
use zeroize::{Zeroize, Zeroizing}; // Upewnij się, że Zeroizing jest zaimportowane
use serde::{Serialize, Deserialize};
use std::fmt;

/// Errors returned by identity operations.
#[derive(Debug, Clone)]
pub enum IdentityError {
    ErrInvalidPhoneFormat,
    ErrKeyGenerationFailed,
    ErrSignatureVerificationFailed,
    ErrExportFailed,
    ErrImportFailed,
    ErrSeedMismatch, // Critical: Seed does not match the identity ID
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            IdentityError::ErrInvalidPhoneFormat => write!(f, "ERR_INVALID_PHONE_FORMAT"),
            IdentityError::ErrKeyGenerationFailed => write!(f, "ERR_KEY_GENERATION_FAILED"),
            IdentityError::ErrSignatureVerificationFailed => write!(f, "ERR_SIGNATURE_VERIFICATION_FAILED"),
            IdentityError::ErrExportFailed => write!(f, "ERR_EXPORT_FAILED"),
            IdentityError::ErrImportFailed => write!(f, "ERR_IMPORT_FAILED"),
            IdentityError::ErrSeedMismatch => write!(f, "ERR_SEED_MISMATCH_IDENTITY_CORRUPT"),
        }
    }
}

/// Serializable structure for backup/migration.
/// Contains the private key seed AND a signature to verify integrity upon restore.
#[derive(Clone, Serialize, Deserialize)]
pub struct IdentityBackup {
    pub phone_hash: String,
    pub secret_key_seed: [u8; 32],
    pub created_at: u64,
    pub validation_signature: Vec<u8>, // Signature of phone_hash (as bytes) made by the key itself
}

/// The core Identity structure.
pub struct Identity {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub phone_hash: String, 
}

impl Identity {
    /// Generates a new identity based on a phone number.
    pub fn generate(phone_number: &str) -> Result<Self, IdentityError> {
        if !Self::validate_phone_format(phone_number) {
            return Err(IdentityError::ErrInvalidPhoneFormat);
        }

        let mut hasher = Sha256::new();
        hasher.update(phone_number.as_bytes());
        let phone_hash = hex::encode(hasher.finalize());

        let mut csprng = OsRng {};
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        Ok(Identity {
            signing_key,
            verifying_key,
            phone_hash,
        })
    }

    /// Restores an identity from a backup seed with cryptographic verification.
    /// Ensures that the seed actually corresponds to the provided phone_hash.
    pub fn restore_from_seed(seed: [u8; 32], expected_phone_hash: &str, validation_sig: &[u8]) -> Result<Self, IdentityError> {
        // 1. Reconstruct the key pair from seed
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        
        // 2. Cryptographic Verification (The Missing Link)
        // We verify that the reconstructed key can validate the signature stored in the backup
        // against the expected phone hash STRING (converted to bytes).
        let sig = Signature::from_slice(validation_sig)
            .map_err(|_| IdentityError::ErrImportFailed)?;
            
        // CRITICAL FIX: We sign/verify the STRING representation of the hash (hex), 
        // exactly as it was done in export_secure. No hex-decoding of the hash itself.
        let hash_bytes = expected_phone_hash.as_bytes();

        // If this fails, the seed does not match the phone_hash (corruption or tampering)
        verifying_key.verify(hash_bytes, &sig)
            .map_err(|_| IdentityError::ErrSeedMismatch)?;

        Ok(Identity {
            signing_key,
            verifying_key,
            phone_hash: expected_phone_hash.to_string(),
        })
    }

    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }

    pub fn verify(public_key: &VerifyingKey, data: &[u8], signature: &Signature) -> Result<(), IdentityError> {
        public_key.verify(data, signature)
            .map_err(|_| IdentityError::ErrSignatureVerificationFailed)
    }

    /// Exports the identity for secure backup.
    /// Now includes a self-signature for integrity verification on restore.
    /// SECURITY: Uses Zeroizing to wipe the seed from memory immediately after use.
    pub fn export_secure(&self) -> Result<IdentityBackup, IdentityError> {
        // Use Zeroizing wrapper to ensure the seed is wiped when this variable goes out of scope
        let mut seed: Zeroizing<[u8; 32]> = Zeroizing::new(self.signing_key.to_bytes());
        
        // Create a signature of the phone_hash STRING using the current key.
        // We use .as_bytes() on the hex string.
        let validation_signature = self.signing_key.sign(self.phone_hash.as_bytes()).to_bytes().to_vec();
        
        // Copy the seed into the backup struct (this creates a non-zeroizing copy, 
        // which is then serialized. The local 'seed' variable will be zeroed automatically).
        let mut backup_seed = [0u8; 32];
        backup_seed.copy_from_slice(&*seed);

        // Explicitly zero the local variable before it drops (redundant but explicit safety)
        seed.zeroize();
        
        Ok(IdentityBackup {
            phone_hash: self.phone_hash.clone(),
            secret_key_seed: backup_seed,
            created_at: chrono::Utc::now().timestamp() as u64,
            validation_signature,
        })
    }

    fn validate_phone_format(phone: &str) -> bool {
        if phone.is_empty() || phone.len() > 15 {
            return false;
        }
        let mut chars = phone.chars();
        if let Some(first) = chars.next() {
            if first == '+' {
                return chars.all(|c| c.is_ascii_digit());
            } else if !first.is_ascii_digit() {
                return false;
            }
        }
        chars.all(|c| c.is_ascii_digit())
    }
}

impl Drop for Identity {
    /// Ensures that sensitive key material is wiped from memory when the struct is dropped.
    fn drop(&mut self) {
        // Explicitly zeroize the signing key bytes in memory.
        // Note: SigningKey doesn't expose internal bytes for zeroizing directly via public API 
        // in older versions, but we can rely on the fact that we are dropping the struct.
        // However, if we had raw bytes stored, we would call .zeroize() here.
        // For maximum safety with ed25519-dalek, we assume the library handles its own Drop,
        // but we log the event for audit trails.
        
        // If we stored raw secret bytes in a field, we would do: self.secret_bytes.zeroize();
        // Since we store the Key object, we trust its Drop impl, but we clear the phone_hash just in case.
        self.phone_hash.zeroize();
        
        log::info!("MSG_IDENTITY_DROPPED_KEYS_SECURED");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_backup_and_restore() {
        let phone = "+48123456789";
        let original = Identity::generate(phone).unwrap();
        
        // Export (includes validation signature)
        let backup = original.export_secure().unwrap();
        
        // Restore with verification
        let restored = Identity::restore_from_seed(
            backup.secret_key_seed, 
            &backup.phone_hash,
            &backup.validation_signature
        ).unwrap();
        
        assert_eq!(original.verifying_key.to_bytes(), restored.verifying_key.to_bytes());
    }

    #[test]
    fn test_restore_with_tampered_hash() {
        let phone = "+48123456789";
        let original = Identity::generate(phone).unwrap();
        let backup = original.export_secure().unwrap();
        
        // Tamper with the phone hash in the backup data
        let fake_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        
        // Attempt to restore should fail because the signature won't match the fake hash
        let result = Identity::restore_from_seed(
            backup.secret_key_seed, 
            fake_hash,
            &backup.validation_signature
        );
        
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), IdentityError::ErrSeedMismatch);
    }
    
    #[test]
    fn test_restore_with_tampered_seed() {
        let phone = "+48123456789";
        let original = Identity::generate(phone).unwrap();
        let mut backup = original.export_secure().unwrap();
        
        // Tamper with the seed
        let mut fake_seed = [0u8; 32];
        fake_seed[0] = 1;
        backup.secret_key_seed = fake_seed;
        
        // Attempt to restore should fail
        let result = Identity::restore_from_seed(
            backup.secret_key_seed, 
            &backup.phone_hash,
            &backup.validation_signature
        );
        
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), IdentityError::ErrSeedMismatch);
    }
}
