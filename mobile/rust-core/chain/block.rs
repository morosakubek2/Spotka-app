// mobile/rust-core/src/chain/transaction.rs
// Transaction Definitions & Validation Logic for App-Chain.
// Privacy: Stores only Phone Hashes (SHA-256), never raw numbers.
// Year: 2026 | Rust Edition: 2024

use serde::{Serialize, Deserialize};
use ed25519_dalek::{Signature, VerifyingKey, Signer, SigningKey};
use crate::dict::cts_parser; // Import our CTS parser logic

/// Types of transactions supported by the Spotka App-Chain.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TxType {
    /// Issuing a trust certificate (Web of Trust).
    TrustIssue,
    /// Revoking trust (Requires Quorum of signatures).
    TrustRevoke,
    /// Updating reputation score (Attendance vs No-Show).
    RepUpdate,
    /// Publishing meetup metadata (Includes validated CTS tags).
    MeetupMeta,
}

/// Payload specific to Trust Revocation.
/// Crucial for Anti-Sybil: Requires multiple independent signatures.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevokePayload {
    pub target_user_hash: String, // Phone Hash of the user being revoked
    pub reason_code: String,      // Error key (e.g., "ERR_FAKE_PROFILE")
    pub quorum_signatures: Vec<(String, Vec<u8>)>, // (ValidatorID, Signature)
}

/// Payload specific to Meetup Metadata.
/// Ensures CTS tags are valid before entering the chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetupPayload {
    pub organizer_hash: String,
    pub location_lat: f64,
    pub location_lon: f64,
    pub start_time: u64,
    pub min_duration_mins: u32,
    pub cts_tags: String, // The raw CTS string (e.g., "kino0alkohol")
    pub guest_count: u8,
}

/// The core Transaction structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String, // UUID v4
    pub tx_type: TxType,
    pub timestamp: u64,
    
    // Data payload (serialized bincode for compactness)
    pub raw_ Vec<u8>,
    
    // Cryptographic proof
    pub initiator_signature: Vec<u8>, // Ed25519 signature by the creator
    
    // Retention hint: True if this tx involves a low-rep user (keep longer)
    pub extended_retention: bool, 
}

impl Transaction {
    /// Creates a new TrustIssue transaction.
    pub fn new_trust_issue(
        id: String,
        target_hash: String,
        signer: &SigningKey,
        timestamp: u64,
    ) -> Result<Self, &'static str> {
        let payload = bincode::serialize(&target_hash).map_err(|_| "ERR_SERIALIZE_FAILED")?;
        
        let mut tx = Transaction {
            id,
            tx_type: TxType::TrustIssue,
            timestamp,
            raw_ payload,
            initiator_signature: vec![],
            extended_retention: false,
        };
        
        tx.sign(signer)?;
        Ok(tx)
    }

    /// Creates a new TrustRevoke transaction with Quorum support.
    pub fn new_trust_revoke(
        id: String,
        payload: RevokePayload,
        signer: &SigningKey,
        timestamp: u64,
    ) -> Result<Self, &'static str> {
        // Validation: Must have at least 3 quorum signatures
        if payload.quorum_signatures.len() < 3 {
            return Err("ERR_REVOKE_QUORUM_MISSING");
        }

        let data = bincode::serialize(&payload).map_err(|_| "ERR_SERIALIZE_FAILED")?;
        
        let mut tx = Transaction {
            id,
            tx_type: TxType::TrustRevoke,
            timestamp,
            raw_ data,
            initiator_signature: vec![],
            extended_retention: true, // Revocations are critical, keep longer
        };
        
        tx.sign(signer)?;
        Ok(tx)
    }

    /// Creates a new MeetupMeta transaction with CTS validation.
    pub fn new_meetup_meta(
        id: String,
        payload: MeetupPayload,
        signer: &SigningKey,
        timestamp: u64,
    ) -> Result<Self, &'static str> {
        // CRITICAL: Validate CTS tags before committing to chain
        // This prevents invalid tag structures from polluting the ledger
        if let Err(_err_code) = cts_parser::parse_cts(&payload.cts_tags) {
            // Return the error key (Language Agnostic)
            return Err("ERR_INVALID_CTS_SEQUENCE"); 
        }

        let data = bincode::serialize(&payload).map_err(|_| "ERR_SERIALIZE_FAILED")?;
        
        let mut tx = Transaction {
            id,
            tx_type: TxType::MeetupMeta,
            timestamp,
            raw_ data,
            initiator_signature: vec![],
            // Check if organizer has low rep (logic omitted, assumed false for new meetups)
            extended_retention: false, 
        };
        
        tx.sign(signer)?;
        Ok(tx)
    }

    /// Signs the transaction with the initiator's private key.
    fn sign(&mut self, key: &SigningKey) -> Result<(), &'static str> {
        let msg = self.get_signing_message();
        let sig = key.sign(&msg);
        self.initiator_signature = sig.to_bytes().to_vec();
        Ok(())
    }

    /// Verifies the initiator's signature.
    pub fn verify_signature(&self, pub_key: &VerifyingKey) -> bool {
        let msg = self.get_signing_message();
        if self.initiator_signature.len() != 64 {
            return false;
        }
        let sig = match Signature::from_bytes(self.initiator_signature.as_slice().try_into().unwrap()) {
            Ok(s) => s,
            Err(_) => return false,
        };
        pub_key.verify(&msg, &sig).is_ok()
    }

    /// Helper to get bytes for signing (hash of type + timestamp + payload)
    fn get_signing_message(&self) -> Vec<u8> {
        let mut msg = Vec::new();
        msg.extend_from_slice(format!("{:?}", self.tx_type).as_bytes());
        msg.extend_from_slice(&self.timestamp.to_le_bytes());
        msg.extend_from_slice(&self.raw_);
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_invalid_cts_rejected() {
        let mut csprng = OsRng {};
        let key = SigningKey::generate(&mut csprng);
        
        // Invalid CTS: Two positive tags ("kino" and "teatr")
        let payload = MeetupPayload {
            organizer_hash: "hash123".to_string(),
            location_lat: 52.0,
            location_lon: 21.0,
            start_time: 1000,
            min_duration_mins: 60,
            cts_tags: "kinoteatr".to_string(), // Missing separator/status
            guest_count: 0,
        };

        let result = Transaction::new_meetup_meta(
            "tx1".to_string(),
            payload,
            &key,
            1000,
        );

        assert!(result.is_err());
        assert_eq!(result.err(), Some("ERR_INVALID_CTS_SEQUENCE"));
    }

    #[test]
    fn test_revoke_quorum_enforced() {
        let mut csprng = OsRng {};
        let key = SigningKey::generate(&mut csprng);
        
        let payload = RevokePayload {
            target_user_hash: "bad_actor".to_string(),
            reason_code: "ERR_FAKE_PROFILE".to_string(),
            quorum_signatures: vec![], // Empty!
        };

        let result = Transaction::new_trust_revoke(
            "tx2".to_string(),
            payload,
            &key,
            1000,
        );

        assert!(result.is_err());
        assert_eq!(result.err(), Some("ERR_REVOKE_QUORUM_MISSING"));
    }
}
