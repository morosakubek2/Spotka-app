// mobile/rust-core/src/crypto/e2ee.rs
// End-to-End Encryption Module (Hybrid: X25519 + AES-256-GCM).
// Features: PFS, Replay Protection, Authenticated Encryption, Memory Safety.
// Year: 2026 | Rust Edition: 2024

use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use sha2::{Sha256, Digest};
use rand::rngs::OsRng;
use zeroize::{Zeroize, Zeroizing};
use serde::{Serialize, Deserialize};
use log::{error, warn};

/// Encrypted Payload structure sent over P2P.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedPacket {
    pub ephemeral_pub_key: Vec<u8>, // Public key of the sender's ephemeral keypair (for DH)
    pub nonce: Vec<u8>,             // 12-byte nonce for AES-GCM
    pub ciphertext: Vec<u8>,        // Encrypted (and authenticated) data
    pub timestamp: u64,             // Unix timestamp for replay protection
}

/// Result of decryption.
pub struct DecryptedPayload {
    pub data: Vec<u8>,
    pub sender_pub_key: PublicKey,
}

/// Main encryption/decryption handler.
pub struct E2EE;

impl E2EE {
    /// Encrypts data for a specific recipient using their long-term public key.
    /// Implements Hybrid Encryption:
    /// 1. Generate ephemeral X25519 keypair.
    /// 2. Derive shared secret (DH).
    /// 3. Derive AES key from shared secret + context (HKDF-like via SHA256).
    /// 4. Encrypt with AES-256-GCM.
    pub fn encrypt(
        data: &[u8],
        recipient_long_term_pub_key: &PublicKey,
        sender_long_term_priv_key: &EphemeralSecret, // Usually passed as reference to secret
    ) -> Result<EncryptedPacket, &'static str> {
        // 1. Generate Ephemeral Key for Perfect Forward Secrecy
        let csprng = OsRng {};
        let ephemeral_secret = EphemeralSecret::random_from_rng(csprng);
        let ephemeral_pub_key = ephemeral_secret.clamp().to_public_key(); // Note: clamp() is crucial for X25519

        // 2. Diffie-Hellman Key Exchange
        // Shared Secret = Ephemeral_Secret * Recipient_Public_Key
        // Note: In a real handshake, we might use static-ephemeral or ephemeral-ephemeral.
        // Here we assume sender uses ephemeral, recipient uses static long-term key.
        let shared_secret = ephemeral_secret.diffie_hellman(recipient_long_term_pub_key);

        // 3. Derive AES Key (32 bytes) from Shared Secret
        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        // Add context info to prevent key reuse in different protocols
        hasher.update(b"SPOTKA_E2EE_V1"); 
        let key_bytes = hasher.finalize();
        
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|_| "ERR_INVALID_KEY_SIZE")?;

        // 4. Generate Random Nonce (12 bytes for GCM)
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // 5. Encrypt (Authenticated Encryption)
        let payload = Payload {
            msg: data,
            aad: b"", // Additional Authenticated Data (empty for now)
        };

        let ciphertext = cipher
            .encrypt(nonce, payload)
            .map_err(|_| "ERR_ENCRYPTION_FAILED")?;

        Ok(EncryptedPacket {
            ephemeral_pub_key: ephemeral_pub_key.as_bytes().to_vec(),
            nonce: nonce_bytes.to_vec(),
            ciphertext,
            timestamp: chrono::Utc::now().timestamp() as u64,
        })
    }

    /// Decrypts a packet using the recipient's long-term private key.
    /// Verifies integrity and checks for replay attacks.
    pub fn decrypt(
        packet: &EncryptedPacket,
        recipient_long_term_priv_key: &EphemeralSecret,
    ) -> Result<DecryptedPayload, &'static str> {
        
        // 1. Replay Attack Protection (Time Window)
        let now = chrono::Utc::now().timestamp() as u64;
        let max_age_seconds = 300; // 5 minutes window
        if now > packet.timestamp + max_age_seconds {
            warn!("ERR_PACKET_EXPIRED");
            return Err("ERR_PACKET_EXPIRED");
        }

        // 2. Reconstruct Sender's Public Key
        let sender_pub_key_bytes: [u8; 32] = packet.ephemeral_pub_key
            .clone()
            .try_into()
            .map_err(|_| "ERR_INVALID_PUB_KEY_LENGTH")?;
        
        let sender_pub_key = PublicKey::from(sender_pub_key_bytes);

        // 3. Diffie-Hellman Key Exchange (Reverse side)
        // Shared Secret = Recipient_Private_Key * Sender_Ephemeral_Public_Key
        let shared_secret = recipient_long_term_priv_key.diffie_hellman(&sender_pub_key);

        // 4. Derive AES Key (Must match sender's derivation)
        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(b"SPOTKA_E2EE_V1");
        let key_bytes = hasher.finalize();

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|_| "ERR_INVALID_KEY_SIZE")?;

        // 5. Decrypt & Verify Integrity
        let nonce = Nonce::from_slice(&packet.nonce);
        let payload = Payload {
            msg: &packet.ciphertext,
            aad: b"",
        };

        let plaintext = cipher
            .decrypt(nonce, payload)
            .map_err(|_| "ERR_DECRYPTION_FAILED_OR_TAMPERED")?; // Failure means either wrong key or tampered data

        Ok(DecryptedPayload {
            data: plaintext,
            sender_pub_key,
        })
    }

    /// Helper to compress data before encryption (Optional optimization).
    #[cfg(feature = "compression")]
    pub fn compress_and_encrypt(
        data: &[u8],
        recipient_pub_key: &PublicKey,
        sender_priv_key: &EphemeralSecret,
    ) -> Result<EncryptedPacket, &'static str> {
        use lz4_flex::compress_prepend_size;
        let compressed = compress_prepend_size(data);
        Self::encrypt(&compressed, recipient_pub_key, sender_priv_key)
    }
}

// Ensure secrets are wiped from memory when dropped
impl Drop for EphemeralSecret {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x25519_dalek::EphemeralSecret;

    #[test]
    fn test_round_trip_encryption() {
        // Generate Long-term Keys for Recipient
        let recipient_secret = EphemeralSecret::random_from_rng(OsRng);
        let recipient_pub = recipient_secret.clamp().to_public_key();

        // Generate Long-term Keys for Sender (simulated as ephemeral for test simplicity)
        let sender_secret = EphemeralSecret::random_from_rng(OsRng);
        
        let original_data = b"Secret Meetup Location: Park Bench";

        // Encrypt
        let packet = E2EE::encrypt(original_data, &recipient_pub, &sender_secret)
            .expect("Encryption failed");

        // Decrypt
        let result = E2EE::decrypt(&packet, &recipient_secret)
            .expect("Decryption failed");

        assert_eq!(result.data, original_data);
    }

    #[test]
    fn test_tampering_detection() {
        let recipient_secret = EphemeralSecret::random_from_rng(OsRng);
        let recipient_pub = recipient_secret.clamp().to_public_key();
        let sender_secret = EphemeralSecret::random_from_rng(OsRng);

        let packet = E2EE::encrypt(b"Original Data", &recipient_pub, &sender_secret).unwrap();

        // Tamper with ciphertext
        let mut tampered_packet = packet.clone();
        tampered_packet.ciphertext[0] ^= 0xFF; 

        let result = E2EE::decrypt(&tampered_packet, &recipient_secret);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "ERR_DECRYPTION_FAILED_OR_TAMPERED");
    }
}
