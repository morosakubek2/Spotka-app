// mobile/rust-core/src/chain/transaction.rs
// Defines specific transaction types for the "Anti-Social" trust model.
// NO personal data. Only hashes and cryptographic proofs.

use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

/// Types of transactions allowed on App-Chain.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TxType {
    /// Issuing a trust certificate (User A verifies User B).
    TrustIssue,
    /// Revoking trust (requires Quorum signatures).
    TrustRevoke,
    /// Updating reputation score after a meetup (No-Show vs Attended).
    RepUpdate,
    /// Metadata hash of a meetup (Time, Location Hash, Tags Hash) - NO content.
    MeetupMeta,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String, // UUID or Hash of content
    pub tx_type: TxType,
    pub timestamp: u64,
    
    // The core payload is opaque binary data, interpreted based on TxType.
    // E.g., for TrustIssue: { issuer_hash, target_hash, signature }
    pub payload_hash: String, 
    pub raw_ Vec<u8>,
    
    // Aggregated signatures if applicable (e.g., for Revocation Quorum)
    pub signatures: Vec<Vec<u8>>, 
}

impl Transaction {
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bincode::serialize(&self).unwrap());
        hex::encode(hasher.finalize())
    }

    /// Factory method for creating a TrustIssue transaction.
    /// Note: Phone numbers are NEVER passed here, only their SHA256 hashes.
    pub fn new_trust_issue(issuer_phone_hash: &str, target_phone_hash: &str, signature: Vec<u8>) -> Self {
        let payload = format!("{}|{}", issuer_phone_hash, target_phone_hash);
        let raw = payload.as_bytes().to_vec();
        let payload_hash = hex::encode(Sha256::digest(&raw));

        Transaction {
            id: uuid::Uuid::new_v4().to_string(),
            tx_type: TxType::TrustIssue,
            timestamp: chrono::Utc::now().timestamp() as u64,
            payload_hash,
            raw_,
            signatures: vec![signature],
        }
    }

    /// Factory for Revocation (requires multiple signatures/quorum).
    pub fn new_trust_revoke(target_phone_hash: &str, quorum_sigs: Vec<Vec<u8>>) -> Self {
        let raw = target_phone_hash.as_bytes().to_vec();
        let payload_hash = hex::encode(Sha256::digest(&raw));

        Transaction {
            id: uuid::Uuid::new_v4().to_string(),
            tx_type: TxType::TrustRevoke,
            timestamp: chrono::Utc::now().timestamp() as u64,
            payload_hash,
            raw_,
            signatures: quorum_sigs,
        }
    }
}
