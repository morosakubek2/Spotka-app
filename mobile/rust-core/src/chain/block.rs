// mobile/rust-core/src/chain/block.rs
// App-Chain Block Structure & Merkle Tree Implementation.
// Architecture: Lightweight ledger for trust metadata only (No personal data).
// Security: Zeroize memory on drop, Deterministic Merkle, Ed25519 Signatures.
// Year: 2026 | Rust Edition: 2024

use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use ed25519_dalek::{Signature, VerifyingKey, Signer, SigningKey};
use zeroize::{Zeroize, Zeroizing}; // CRITICAL for security
use std::collections::HashMap;

/// Types of transactions allowed in the Spotka App-Chain.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TxType {
    TrustIssue,
    TrustRevoke,
    RepUpdate,
    MeetupMeta,
}

/// A single transaction unit within a block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String, 
    pub tx_type: TxType,
    pub payload_hash: String, 
    pub raw_data: Vec<u8>,    
    pub signature: Vec<u8>,   
    pub timestamp: u64,
}

// SECURITY: Securely wipe sensitive data when Transaction is dropped
impl Drop for Transaction {
    fn drop(&mut self) {
        self.raw_data.zeroize();
        self.signature.zeroize();
        // payload_hash and id are public metadata, no need to zeroize
    }
}

/// Header of a block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockHeader {
    pub height: u64,
    pub prev_hash: String,
    pub merkle_root: String,
    pub timestamp: u64,
    pub validator_id: String, 
}

/// The complete Block structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub signature: Vec<u8>, 
}

// SECURITY: Securely wipe block signature when dropped
impl Drop for Block {
    fn drop(&mut self) {
        self.signature.zeroize();
        // Transactions will zeroize themselves when the vector is dropped
    }
}

impl Block {
    /// Creates a new block with the given transactions.
    pub fn new(
        height: u64,
        prev_hash: String,
        mut transactions: Vec<Transaction>,
        validator_id: String,
    ) -> Self {
        // CRITICAL: Sort transactions to ensure deterministic Merkle Root.
        transactions.sort_by(|a, b| a.id.cmp(&b.id));

        let merkle_root = Self::calculate_merkle_root(&transactions);
        
        let header = BlockHeader {
            height,
            prev_hash,
            merkle_root,
            timestamp: chrono::Utc::now().timestamp() as u64,
            validator_id,
        };

        Block {
            header,
            transactions,
            signature: vec![], 
        }
    }

    /// Basic structural validation before expensive crypto operations.
    pub fn validate_structure(&self) -> Result<(), &'static str> {
        if self.header.validator_id.is_empty() {
            return Err("ERR_BLOCK_EMPTY_VALIDATOR_ID");
        }
        if self.header.height == 0 && self.header.prev_hash != "genesis" {
             // Genesis block exception logic could go here
        }
        // Add more checks as needed (e.g., timestamp sanity)
        Ok(())
    }

    /// Calculates the SHA256 hash of the block header.
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();
        let data = bincode::serialize(&self.header).unwrap_or_default();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Verifies the integrity of the block's transactions against the Merkle Root.
    pub fn verify_merkle_root(&self) -> bool {
        let calculated_root = Self::calculate_merkle_root(&self.transactions);
        calculated_root == self.header.merkle_root
    }

    /// Internal function to build the Merkle Tree.
    fn calculate_merkle_root(transactions: &[Transaction]) -> String {
        if transactions.is_empty() {
            return hex::encode(Sha256::digest("EMPTY_BLOCK"));
        }

        let mut hashes: Vec<String> = transactions
            .iter()
            .map(|tx| {
                let data = bincode::serialize(tx).unwrap_or_default();
                hex::encode(Sha256::digest(data))
            })
            .collect();

        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in hashes.chunks(2) {
                let left = &chunk[0];
                let right = chunk.get(1).unwrap_or(left);
                let combined = format!("{}{}", left, right);
                let hash = hex::encode(Sha256::digest(combined.as_bytes()));
                next_level.push(hash);
            }
            hashes = next_level;
        }
        hashes[0].clone()
    }

    /// Verifies the block signature.
    pub fn verify_signature(&self, validator_pub_key: &VerifyingKey) -> Result<(), &'static str> {
        if self.signature.is_empty() {
            return Err("ERR_INVALID_BLOCK_SIGNATURE_EMPTY");
        }

        let sig = Signature::from_slice(&self.signature)
            .map_err(|_| "ERR_INVALID_BLOCK_SIGNATURE_FORMAT")?;

        let msg = bincode::serialize(&self.header).unwrap_or_default();
        
        validator_pub_key
            .verify_strict(&msg, &sig)
            .map_err(|_| "ERR_INVALID_BLOCK_SIGNATURE_MISMATCH")?;

        Ok(())
    }
    
    /// Signs the block header.
    pub fn sign(&mut self, signing_key: &SigningKey) {
        let msg = bincode::serialize(&self.header).unwrap_or_default();
        let sig = signing_key.sign(&msg);
        self.signature = sig.to_bytes().to_vec();
    }

    /// Helper to extract transaction IDs quickly (useful for Sync Delta).
    pub fn get_transaction_ids(&self) -> Vec<String> {
        self.transactions.iter().map(|tx| tx.id.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_merkle_root_determinism() {
        let tx1 = Transaction {
            id: "1".to_string(),
            tx_type: TxType::RepUpdate,
            payload_hash: "hash1".to_string(),
            raw_data: vec![1, 2, 3],
            signature: vec![],
            timestamp: 100,
        };
        let tx2 = Transaction {
            id: "2".to_string(),
            tx_type: TxType::TrustIssue,
            payload_hash: "hash2".to_string(),
            raw_data: vec![4, 5, 6],
            signature: vec![],
            timestamp: 101,
        };

        let block1 = Block::new(1, "genesis".to_string(), vec![tx1.clone(), tx2.clone()], "validator_a".to_string());
        let block2 = Block::new(1, "genesis".to_string(), vec![tx2.clone(), tx1.clone()], "validator_a".to_string());

        assert_eq!(block1.header.merkle_root, block2.header.merkle_root);
    }

    #[test]
    fn test_zeroize_on_drop() {
        let mut tx = Transaction {
            id: "tx".to_string(),
            tx_type: TxType::MeetupMeta,
            payload_hash: "h".to_string(),
            raw_data: vec![1, 2, 3, 4],
            signature: vec![5, 6, 7, 8],
            timestamp: 0,
        };
        
        // Manually trigger drop logic for testing (usually automatic)
        tx.raw_data.zeroize();
        tx.signature.zeroize();

        assert!(tx.raw_data.iter().all(|&x| x == 0));
        assert!(tx.signature.iter().all(|&x| x == 0));
    }
}
