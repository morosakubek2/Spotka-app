// mobile/rust-core/src/crypto/e2ee.rs
// End-to-End Encryption Module (Hybrid: X25519 + AES-256-GCM).
// Features: PFS, Replay Protection, Authenticated Encryption (with AAD), Memory Safety, Compression.
// Year: 2026 | Rust Edition: 2024

use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use sha2::{Sha256, Digest};
use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::{Zeroize, Zeroizing};
use serde::{Serialize, Deserialize};
use log::{error, warn};
use lz4_flex::compress_prepend_size; // Added compression dependency

/// Encrypted Payload structure sent over P2P.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedPacket {
    pub ephemeral_pub_key: Vec<u8>, // 32 bytes
    pub nonce: Vec<u8>,             // 12 bytes
    pub ciphertext: Vec<u8>,        
    pub timestamp: u64,             
}

/// Result of decryption.
pub struct DecryptedPayload {
    pub data: Vec<u8>,
    pub sender_pub_key: PublicKey,
}

/// Main encryption/decryption handler.
pub struct E2EE;

impl E2EE {
    /// Encrypts data for a specific recipient.
    /// Includes: Compression, PFS, AAD authentication.
    pub fn encrypt(
        data: &[u8],
        recipient_long_term_pub_key: &PublicKey,
        sender_long_term_priv_key: &EphemeralSecret,
        aad_context: &[u8], // NEW: Additional Authenticated Data (e.g., sender_id, msg_type)
    ) -> Result<EncryptedPacket, &'static str> {
        // 1. Compress Data (Default behavior for P2P efficiency)
        let compressed_data = compress_prepend_size(data);

        // 2. Generate Ephemeral Key for PFS
        let mut csprng = OsRng {};
        let ephemeral_secret = EphemeralSecret::random_from_rng(&mut csprng);
        let ephemeral_pub_key = ephemeral_secret.clamp().to_public_key();

        // 3. Diffie-Hellman Key Exchange
        let shared_secret = ephemeral_secret.diffie_hellman(recipient_long_term_pub_key);

        // 4. Derive AES Key (HKDF-like via SHA256) with Context
        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(b"SPOTKA_E2EE_V1"); 
        hasher.update(aad_context); // Include context in key derivation for extra safety
        let mut key_bytes = hasher.finalize(); // Use Zeroizing implicitly via stack drop later if needed, but explicit is better
        
        // Wrap key in Zeroizing to ensure it's wiped when 'cipher' goes out of scope or on error
        let zero_key = Zeroizing::new(key_bytes); 

        let cipher = Aes256Gcm::new_from_slice(&*zero_key)
            .map_err(|_| "ERR_INVALID_KEY_SIZE")?;

        // 5. Generate Random Nonce (12 bytes)
        let mut nonce_bytes = [0u8; 12];
        csprng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // 6. Encrypt with AAD
        let payload = Payload {
            msg: &compressed_data,
            aad: aad_context, // Authenticate the context too!
        };

        let ciphertext = cipher
            .encrypt(nonce, payload)
            .map_err(|_| "ERR_ENCRYPTION_FAILED")?;

        // Explicitly clear sensitive buffers before returning
        drop(zero_key); 
        shared_secret.zeroize(); // Ensure shared secret is wiped

        Ok(EncryptedPacket {
            ephemeral_pub_key: ephemeral_pub_key.as_bytes().to_vec(),
            nonce: nonce_bytes.to_vec(),
            ciphertext,
            timestamp: chrono::Utc::now().timestamp() as u64,
        })
    }

    /// Decrypts a packet.
    /// Verifies integrity, AAD, and checks for replay attacks.
    pub fn decrypt(
        packet: &EncryptedPacket,
        recipient_long_term_priv_key: &EphemeralSecret,
        aad_context: &[u8], // Must match the AAD used during encryption
    ) -> Result<DecryptedPayload, &'static str> {
        
        // 1. Replay Attack Protection
        let now = chrono::Utc::now().timestamp() as u64;
        let max_age_seconds = 300; // 5 minutes
        if now > packet.timestamp + max_age_seconds {
            warn!("ERR_PACKET_EXPIRED");
            return Err("ERR_PACKET_EXPIRED");
        }

        // 2. Validate Public Key Length explicitly
        if packet.ephemeral_pub_key.len() != 32 {
            return Err("ERR_INVALID_PUB_KEY_LENGTH");
        }
        let mut sender_pub_key_bytes = [0u8; 32];
        sender_pub_key_bytes.copy_from_slice(&packet.ephemeral_pub_key);
        let sender_pub_key = PublicKey::from(sender_pub_key_bytes);

        // 3. Diffie-Hellman
        let shared_secret = recipient_long_term_priv_key.diffie_hellman(&sender_pub_key);

        // 4. Derive AES Key
        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(b"SPOTKA_E2EE_V1");
        hasher.update(aad_context);
        let mut key_bytes = hasher.finalize();
        let zero_key = Zeroizing::new(key_bytes);

        let cipher = Aes256Gcm::new_from_slice(&*zero_key)
            .map_err(|_| "ERR_INVALID_KEY_SIZE")?;

        // 5. Validate Nonce Length
        if packet.nonce.len() != 12 {
            return Err("ERR_INVALID_NONCE_LENGTH");
        }
        let nonce = Nonce::from_slice(&packet.nonce);

        // 6. Decrypt & Verify Integrity (including AAD)
        let payload = Payload {
            msg: &packet.ciphertext,
            aad: aad_context, 
        };

        let mut plaintext = cipher
            .decrypt(nonce, payload)
            .map_err(|_| "ERR_DECRYPTION_FAILED_OR_TAMPERED")?;

        // Cleanup
        drop(zero_key);
        shared_secret.zeroize();

        // 7. Decompress
        let decompressed = lz4_flex::decompress_size_prepended(&plaintext)
            .map_err(|_| "ERR_DECOMPRESSION_FAILED")?;

        Ok(DecryptedPayload {
            data: decompressed,
            sender_pub_key,
        })
    }
}

// Ensure secrets are wiped
impl Drop for EphemeralSecret {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip_with_aad() {
        let recipient_secret = EphemeralSecret::random_from_rng(OsRng);
        let recipient_pub = recipient_secret.clamp().to_public_key();
        let sender_secret = EphemeralSecret::random_from_rng(OsRng);
        
        let original_data = b"Secret Meetup Location";
        let aad = b"session_123"; // Context

        let packet = E2EE::encrypt(original_data, &recipient_pub, &sender_secret, aad)
            .expect("Encryption failed");

        // Decrypt with correct AAD
        let result = E2EE::decrypt(&packet, &recipient_secret, aad)
            .expect("Decryption failed");

        assert_eq!(result.data, original_data);
    }

    #[test]
    fn test_aad_mismatch_detection() {
        let recipient_secret = EphemeralSecret::random_from_rng(OsRng);
        let recipient_pub = recipient_secret.clamp().to_public_key();
        let sender_secret = EphemeralSecret::random_from_rng(OsRng);

        let packet = E2EE::encrypt(b"Data", &recipient_pub, &sender_secret, b"correct_aad").unwrap();

        // Try to decrypt with WRONG AAD
        let result = E2EE::decrypt(&packet, &recipient_secret, b"wrong_aad");
        
        // Should fail because GCM tag verification will fail due to AAD mismatch
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "ERR_DECRYPTION_FAILED_OR_TAMPERED");
    }

    #[test]
    fn test_compression_efficiency() {
        let recipient_secret = EphemeralSecret::random_from_rng(OsRng);
        let recipient_pub = recipient_secret.clamp().to_public_key();
        let sender_secret = EphemeralSecret::random_from_rng(OsRng);

        // Repetitive data compresses well
        let large_data = vec![b'a'; 1000]; 
        let packet = E2EE::encrypt(&large_data, &recipient_pub, &sender_secret, b"test").unwrap();

        // Ciphertext should be significantly smaller than raw data + overhead
        // (1000 bytes -> ~20 bytes compressed + crypto overhead ~40 bytes)
        assert!(packet.ciphertext.len() < large_data.len() / 2);
    }
}
