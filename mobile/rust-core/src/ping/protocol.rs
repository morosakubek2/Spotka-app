// mobile/rust-core/src/ping/protocol.rs
// Payload structure for the Ping (QR Code) handshake.
// Security: Signed by the generator to prevent spoofing.

use serde::{Serialize, Deserialize};
use crate::crypto::identity::Identity;

/// Data encoded inside the QR Code for physical pairing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingPayload {
    /// Public key of the user (Ed25519 verifying key bytes).
    pub public_key: Vec<u8>,
    
    /// Hash of the phone number (unique ID).
    pub phone_hash: String,
    
    /// Display name (optional, can be pseudonym).
    pub display_name: String,
    
    /// Timestamp to prevent replay attacks (valid for ~5 mins).
    pub timestamp: u64,
    
    /// Signature of (public_key + phone_hash + timestamp) by the user's private key.
    pub signature: Vec<u8>,
}

impl PingPayload {
    /// Creates a new payload and signs it with the provided identity.
    pub fn new(identity: &Identity, display_name: &str) -> Result<Self, &'static str> {
        let now = chrono::Utc::now().timestamp() as u64;
        
        // Prepare data to sign
        let mut data_to_sign = identity.verifying_key().to_bytes().to_vec();
        data_to_sign.extend_from_slice(identity.phone_hash.as_bytes());
        data_to_sign.extend_from_slice(&now.to_le_bytes());

        let signature = identity.sign(&data_to_sign).to_bytes().to_vec();

        Ok(PingPayload {
            public_key: identity.verifying_key().to_bytes().to_vec(),
            phone_hash: identity.phone_hash.clone(),
            display_name: display_name.to_string(),
            timestamp: now,
            signature,
        })
    }

    /// Verifies the internal signature.
    pub fn verify(&self) -> Result<(), &'static str> {
        use ed25519_dalek::{VerifyingKey, Signature};

        let vk = VerifyingKey::from_bytes(&self.public_key)
            .map_err(|_| "ERR_INVALID_PUBLIC_KEY")?;

        let mut data_to_sign = self.public_key.clone();
        data_to_sign.extend_from_slice(self.phone_hash.as_bytes());
        data_to_sign.extend_from_slice(&self.timestamp.to_le_bytes());

        let sig = Signature::from_slice(&self.signature)
            .map_err(|_| "ERR_INVALID_SIGNATURE_FORMAT")?;

        vk.verify(&data_to_sign, &sig)
            .map_err(|_| "ERR_SIGNATURE_VERIFICATION_FAILED")
    }

    /// Checks if the payload is not expired (e.g., older than 5 minutes).
    pub fn is_fresh(&self, max_age_secs: u64) -> bool {
        let now = chrono::Utc::now().timestamp() as u64;
        now.saturating_sub(self.timestamp) < max_age_secs
    }
}
