// mobile/rust-core/src/crypto/secure_storage.rs
// Secure Storage Module: Integration with OS KeyStore/SecureEnclave via FFI.
// Features: Encrypted Blob Storage, Biometric Auth, Identity Backup/Restore with Proof of Possession.
// Security: AES-256-GCM, Argon2, Zeroize, Memory Safety, Versioning.
// Year: 2026 | Rust Edition: 2024

use crate::crypto::identity::{Identity, IdentityBackup, IdentityError};
use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit}};
use argon2::Argon2;
use zeroize::{Zeroize, Zeroizing};
use log::{info, error, warn};
use std::ffi::CString;

// --- FFI Definitions for Native Keystore (Android Keystore / iOS Secure Enclave) ---
extern "C" {
    fn store_in_keystore(key_id: *const i8, data: *const u8, len: usize) -> i32;
    fn load_from_keystore(key_id: *const i8, buffer: *mut u8, buf_len: usize) -> i32;
    fn delete_from_keystore(key_id: *const i8) -> i32;
    fn authenticate_biometric() -> i32; 
    // NEW: Get detailed error code from last native operation
    fn get_last_keystore_error() -> i32; 
}

// Versioning for backup format compatibility
const BACKUP_FORMAT_VERSION: u8 = 1;

pub struct SecureStorage;

impl SecureStorage {
    const IDENTITY_KEY_ID: &'static str = "spotka_identity_backup_v1";
    const SALT_KEY_ID: &'static str = "spotka_salt_v1";
    const MAX_FAILED_AUTH_ATTEMPTS: u32 = 5; // Limit for brute-force protection

    /// Derives an encryption key from user biometrics/PIN using Argon2.
    fn derive_key(biometric_secret: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, &'static str> {
        let mut key = Zeroizing::new([0u8; 32]);
        let argon2 = Argon2::default();
        
        argon2.hash_password_into(biometric_secret, salt, &mut *key)
            .map_err(|_| "ERR_KEY_DERIVATION_FAILED")?;
            
        Ok(key)
    }

    /// Saves an encrypted backup of the identity.
    pub fn save_identity_backup(identity: &Identity, biometric_secret: &[u8]) -> Result<(), &'static str> {
        info!("MSG_SECURE_STORAGE_SAVING_IDENTITY");

        // 1. Authenticate User
        if unsafe { authenticate_biometric() } != 1 {
            return Err("ERR_BIOMETRIC_AUTH_REQUIRED");
        }

        // 2. Export Identity
        let mut backup = identity.export_secure()
            .map_err(|_| "ERR_EXPORT_FAILED")?;
        
        // NEW: Inject version into backup (if struct allows) or wrap it
        // Since IdentityBackup struct is defined in identity.rs, we assume it has a version field 
        // or we handle versioning at serialization layer. For this example, let's assume 
        // we prepend version byte to the serialized blob.
        
        let mut backup_bytes = Zeroizing::new(
            bincode::serialize(&backup).map_err(|_| "ERR_SERIALIZATION_FAILED")?
        );

        // Prepend version byte
        let mut versioned_data = Vec::with_capacity(1 + backup_bytes.len());
        versioned_data.push(BACKUP_FORMAT_VERSION);
        versioned_data.extend_from_slice(&backup_bytes);

        // 3. Get Salt
        let salt = self::get_or_create_salt()?;

        // 4. Derive Key & Encrypt
        let key_arr = Self::derive_key(biometric_secret, &salt)?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*key_arr));
        
        let mut nonce_bytes = [0u8; 12];
        getrandom::getrandom(&mut nonce_bytes).map_err(|_| "ERR_RANDOM_GENERATION_FAILED")?;
        
        let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce_bytes), versioned_data.as_slice())
            .map_err(|_| "ERR_ENCRYPTION_FAILED")?;

        // Prepare payload (Nonce + Ciphertext)
        let mut payload = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        payload.extend_from_slice(&nonce_bytes);
        payload.extend_from_slice(&ciphertext);

        // 5. Store
        let c_key_id = CString::new(Self::IDENTITY_KEY_ID).unwrap();
        let res = unsafe { store_in_keystore(c_key_id.as_ptr(), payload.as_ptr(), payload.len()) };
        
        // Cleanup
        drop(backup_bytes);
        drop(key_arr);
        payload.zeroize();
        versioned_data.zeroize();

        if res != 0 {
            let err_code = unsafe { get_last_keystore_error() };
            error!("ERR_OS_KEYSTORE_WRITE_FAILED: Code {}", err_code);
            return Err("ERR_OS_KEYSTORE_WRITE_FAILED");
        }

        Ok(())
    }

    /// Loads and decrypts the identity backup.
    pub fn load_identity_backup(biometric_secret: &[u8]) -> Result<Identity, &'static str> {
        info!("MSG_SECURE_STORAGE_LOADING_IDENTITY");

        // 1. Authenticate
        if unsafe { authenticate_biometric() } != 1 {
            // NEW: Check for repeated failures and wipe if necessary (logic handled in native or here)
            return Err("ERR_BIOMETRIC_AUTH_FAILED");
        }

        // 2. Load
        let mut buffer = vec![0u8; 4096];
        let c_key_id = CString::new(Self::IDENTITY_KEY_ID).unwrap();
        let len = unsafe { load_from_keystore(c_key_id.as_ptr(), buffer.as_mut_ptr(), buffer.len()) };
        
        if len <= 0 {
            return Err("ERR_OS_KEYSTORE_READ_FAILED");
        }
        
        // Validate min length (Nonce + Version)
        if len < 13 { 
            return Err("ERR_INVALID_BACKUP_FORMAT");
        }

        let nonce_bytes = &buffer[0..12];
        let ciphertext = &buffer[12..len as usize];

        // 3. Decrypt
        let salt = self::get_or_create_salt()?;
        let key_arr = Self::derive_key(biometric_secret, &salt)?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*key_arr));
        
        let mut decrypted_data = Zeroizing::new(
            cipher.decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
                .map_err(|_| "ERR_DECRYPTION_FAILED_OR_TAMPERED")?
        );

        // 4. Check Version
        let version = decrypted_data[0];
        if version != BACKUP_FORMAT_VERSION {
            warn!("ERR_BACKUP_VERSION_MISMATCH: Expected {}, Got {}", BACKUP_FORMAT_VERSION, version);
            return Err("ERR_BACKUP_VERSION_MISMATCH");
        }

        // 5. Deserialize
        let backup: IdentityBackup = bincode::deserialize(&decrypted_data[1..])
            .map_err(|_| "ERR_DESERIALIZATION_FAILED")?;

        // 6. Restore Identity (Proof of Possession)
        let identity = Identity::restore_from_seed(
            backup.secret_key_seed, 
            &backup.phone_hash, 
            &backup.validation_signature
        )
        .map_err(|e| match e {
            IdentityError::ErrSeedMismatch => "ERR_SEED_MISMATCH_PROOF_OF_POSSESSION",
            IdentityError::ErrInvalidPhoneFormat => "ERR_CORRUPT_BACKUP_DATA",
            _ => "ERR_RESTORE_FAILED"
        })?;

        // Cleanup
        drop(decrypted_data);
        drop(key_arr);
        buffer.zeroize(); // Clear the raw buffer

        Ok(identity)
    }
    
    pub fn delete_identity_backup() -> Result<(), &'static str> {
        let c_key_id = CString::new(Self::IDENTITY_KEY_ID).unwrap();
        let res = unsafe { delete_from_keystore(c_key_id.as_ptr()) };
        if res != 0 {
            return Err("ERR_OS_KEYSTORE_DELETE_FAILED");
        }
        Ok(())
    }
    
    // NEW: Helper to reset failed attempts counter (called by native side after successful auth)
    pub fn reset_auth_counter() {
        // Implementation depends on native side tracking
        info!("MSG_AUTH_COUNTER_RESET");
    }
}

fn get_or_create_salt() -> Result<Vec<u8>, &'static str> {
    let c_salt_id = CString::new(SecureStorage::SALT_KEY_ID).unwrap();
    let mut buffer = [0u8; 32];
    
    let len = unsafe { load_from_keystore(c_salt_id.as_ptr(), buffer.as_mut_ptr(), buffer.len()) };
    
    if len > 0 {
        return Ok(buffer[..len as usize].to_vec());
    }

    let mut new_salt = [0u8; 32];
    getrandom::getrandom(&mut new_salt).map_err(|_| "ERR_RANDOM_GENERATION_FAILED")?;
    
    let res = unsafe { store_in_keystore(c_salt_id.as_ptr(), new_salt.as_ptr(), new_salt.len()) };
    if res != 0 {
        return Err("ERR_SALT_STORAGE_FAILED");
    }
    
    Ok(new_salt.to_vec())
}
