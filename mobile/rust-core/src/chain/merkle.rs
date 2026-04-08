// mobile/rust-core/src/chain/merkle.rs
// Merkle Tree Implementation for App-Chain Data Integrity.
// Features: Inclusion Proofs (SPV), Memory Optimization, Deterministic Sorting, Empty Root Handling.
// Year: 2026 | Rust Edition: 2024

use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use zeroize::Zeroize; // For secure clearing of temporary hashes if needed

/// A single step in the Merkle Proof path.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleStep {
    pub sibling_hash: String,
    pub position: Position,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Position {
    Left,  // Sibling is on the left
    Right, // Sibling is on the right
}

/// The complete Merkle Proof required to verify a leaf's inclusion in the root.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf_hash: String,
    pub root_hash: String,
    pub steps: Vec<MerkleStep>,
}

/// Utility struct for building and verifying Merkle Trees.
pub struct MerkleTree;

impl MerkleTree {
    /// Standard hash for an empty tree (Genesis or empty block).
    pub fn empty_root() -> String {
        hex::encode(Sha256::digest("EMPTY_MERKLE_ROOT_CONSTANT"))
    }

    /// Calculates the Merkle Root from a list of transaction hashes.
    /// Automatically handles odd numbers of leaves by duplicating the last one.
    /// Sorts input to ensure deterministic roots regardless of insertion order.
    pub fn calculate_root(leaves: &[String]) -> String {
        if leaves.is_empty() {
            return Self::empty_root();
        }

        // 1. Sort leaves for determinism (Critical for consensus)
        let mut sorted_leaves = leaves.to_vec();
        sorted_leaves.sort();

        // 2. Build tree level by level with pre-allocated capacity
        let mut current_level = sorted_leaves;

        while current_level.len() > 1 {
            let next_len = (current_level.len() + 1) / 2;
            let mut next_level = Vec::with_capacity(next_len);
            
            // Process pairs
            for chunk in current_level.chunks(2) {
                let left = &chunk[0];
                // If odd number of nodes, duplicate the last one (standard practice)
                let right = chunk.get(1).unwrap_or(left);

                let combined = format!("{}{}", left, right);
                let hash = hex::encode(Sha256::digest(combined.as_bytes()));
                next_level.push(hash);
            }
            
            // Optional: Clear previous level memory explicitly if sensitive (though hashes are public here)
            // current_level.zeroize(); 
            current_level = next_level;
        }

        current_level.into_iter().next().unwrap_or_else(|| Self::empty_root())
    }

    /// Generates a Merkle Proof for a specific leaf at the given index.
    /// Returns None if the leaf is not found or index is out of bounds.
    pub fn generate_proof(leaves: &[String], target_index: usize) -> Option<MerkleProof> {
        if leaves.is_empty() {
            return None;
        }

        // Sort leaves first to match the root calculation logic
        let mut sorted_leaves = leaves.to_vec();
        sorted_leaves.sort();
        
        // Find the actual index after sorting (if input was unsorted, this maps logical to physical)
        // Note: In real usage, 'target_index' should refer to the sorted position or we search by value.
        // Here we assume target_index refers to the position in the SORTED list for simplicity of proof generation.
        if target_index >= sorted_leaves.len() {
            return None;
        }

        let target_leaf = sorted_leaves[target_index].clone();
        let mut current_index = target_index;
        let mut current_level = sorted_leaves;
        let mut steps = Vec::new();

        while current_level.len() > 1 {
            let mut next_level = Vec::with_capacity((current_level.len() + 1) / 2);
            
            for (i, chunk) in current_level.chunks(2).enumerate() {
                let left = &chunk[0];
                let right = chunk.get(1).unwrap_or(left);
                
                // Determine sibling and position relative to our target
                if i * 2 == current_index {
                    // Target is the left node, sibling is right
                    steps.push(MerkleStep {
                        sibling_hash: right.clone(),
                        position: Position::Right,
                    });
                } else if i * 2 + 1 == current_index {
                    // Target is the right node, sibling is left
                    steps.push(MerkleStep {
                        sibling_hash: left.clone(),
                        position: Position::Left,
                    });
                }
                
                let combined = format!("{}{}", left, right);
                next_level.push(hex::encode(Sha256::digest(combined.as_bytes())));
            }

            // Update index for the next level (parent index)
            current_index /= 2;
            current_level = next_level;
        }

        let root = current_level.into_iter().next().unwrap_or_else(|| Self::empty_root());

        Some(MerkleProof {
            leaf_hash: target_leaf,
            root_hash: root,
            steps,
        })
    }

    /// Verifies a Merkle Proof against a known root hash.
    /// Returns true if the proof is valid and reconstructs the root correctly.
    pub fn verify_proof(proof: &MerkleProof) -> bool {
        let mut current_hash = proof.leaf_hash.clone();

        for step in &proof.steps {
            let combined = match step.position {
                Position::Left => format!("{}{}", step.sibling_hash, current_hash),
                Position::Right => format!("{}{}", current_hash, step.sibling_hash),
            };
            current_hash = hex::encode(Sha256::digest(combined.as_bytes()));
        }

        current_hash == proof.root_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tree() {
        let leaves: Vec<String> = vec![];
        let root = MerkleTree::calculate_root(&leaves);
        assert_eq!(root, MerkleTree::empty_root());
    }

    #[test]
    fn test_single_leaf() {
        let leaves = vec!["hash_a".to_string()];
        let root = MerkleTree::calculate_root(&leaves);
        assert_ne!(root, MerkleTree::empty_root());
        
        let proof = MerkleTree::generate_proof(&leaves, 0).unwrap();
        assert!(proof.steps.is_empty());
        assert!(MerkleTree::verify_proof(&proof));
    }

    #[test]
    fn test_odd_leaves_duplication() {
        let leaves = vec![
            "hash_a".to_string(),
            "hash_b".to_string(),
            "hash_c".to_string(),
        ];
        let root = MerkleTree::calculate_root(&leaves);
        
        // Verify determinism
        let mut reversed = leaves.clone();
        reversed.reverse();
        let root_reversed = MerkleTree::calculate_root(&reversed);
        
        assert_eq!(root, root_reversed, "Root must be deterministic regardless of order");
    }

    #[test]
    fn test_proof_verification_and_tampering() {
        let leaves = vec![
            "tx_1".to_string(),
            "tx_2".to_string(),
            "tx_3".to_string(),
            "tx_4".to_string(),
        ];
        
        let proof = MerkleTree::generate_proof(&leaves, 1).unwrap(); // Proof for tx_2
        assert!(MerkleTree::verify_proof(&proof));

        // Tamper with leaf
        let mut bad_proof = proof.clone();
        bad_proof.leaf_hash = "tampered_tx".to_string();
        assert!(!MerkleTree::verify_proof(&bad_proof));

        // Tamper with root
        let mut bad_root_proof = proof;
        bad_root_proof.root_hash = "fake_root".to_string();
        assert!(!MerkleTree::verify_proof(&bad_root_proof));
    }
}
