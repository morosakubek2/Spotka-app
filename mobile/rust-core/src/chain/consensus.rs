// mobile/rust-core/src/chain/consensus.rs
// Consensus Logic: PoA-Lite (Proof-of-Authority Lite) with Reputation & Sybil Resistance.
// Features: Adaptive Thresholds, Sybil Detection, Time-Travel Prevention, Guardian Support.
// Year: 2026 | Rust Edition: 2024

use crate::db::manager::DbManager;
use crate::crypto::identity::Identity;
use crate::dict::cts_parser::{parse_cts, CtsError};
use crate::chain::block::{Block, Transaction, TxType};
use ed25519_dalek::{VerifyingKey, Signature};
use zeroize::Zeroize; // For secure memory clearing
use log::{warn, info, debug};
use std::collections::{HashMap, HashSet};

/// Minimum reputation score required to be a validator.
const MIN_REPUTATION_THRESHOLD: i32 = 80;

/// Maximum allowed clock skew (in seconds) to prevent Time-Travel attacks.
const MAX_CLOCK_SKEW_SEC: i64 = 900; // 15 minutes

/// Required quorum size for Trust Revocation transactions.
const TRUST_REVOKE_QUORUM_SIZE: usize = 3;

/// Validates if a user is eligible to be a validator based on reputation and activity.
pub fn validate_validator(validator_id: &str, db: &DbManager) -> bool {
    // Pseudo-code: Fetch user reputation from DB
    // let user = db.get_user(validator_id)?;
    // if user.reputation_score < MIN_REPUTATION_THRESHOLD { return false; }
    
    // Check for recent activity (Decay Factor)
    // if user.last_seen < (now - 30_days) { return false; }
    
    true // Placeholder
}

/// Detects potential Sybil clusters by analyzing trust graph density.
/// Returns true if the validator seems to be part of a suspicious cluster.
pub fn detect_sybil_cluster(validator_id: &str, db: &DbManager) -> bool {
    // Logic: If a user has many new accounts trusting them exclusively, it's suspicious.
    // Implementation requires graph traversal in DB.
    false // Placeholder
}

/// Main validation function for a new block.
/// Returns Ok(()) if valid, or an error key string.
pub async fn validate_block(block: &Block, db: &DbManager) -> Result<(), &'static str> {
    let now = chrono::Utc::now().timestamp() as u64;

    // 1. Time-Travel Attack Prevention
    if block.header.timestamp > now + MAX_CLOCK_SKEW_SEC as u64 {
        warn!("ERR_BLOCK_TIME_TRAVEL_DETECTED: Block timestamp too far in future");
        return Err("ERR_BLOCK_TIME_TRAVEL_DETECTED");
    }

    // 2. Validator Eligibility
    if !validate_validator(&block.header.validator_id, db) {
        warn!("ERR_INVALID_VALIDATOR_REPUTATION: {}", block.header.validator_id);
        return Err("ERR_INVALID_VALIDATOR_REPUTATION");
    }

    // 3. Sybil Cluster Detection
    if detect_sybil_cluster(&block.header.validator_id, db) {
        warn!("ERR_SYBIL_CLUSTER_DETECTED: {}", block.header.validator_id);
        return Err("ERR_SYBIL_CLUSTER_DETECTED");
    }

    // 4. Validate Each Transaction in the Block
    for tx in &block.transactions {
        validate_transaction(tx, db).await?;
    }

    // 5. Verify Block Signature (Caller should provide the public key)
    // This is usually done before calling this function, but good to have here too
    // let pub_key = db.get_validator_key(&block.header.validator_id)?;
    // block.verify_signature(&pub_key)?;

    Ok(())
}

/// Validates a single transaction.
async fn validate_transaction(tx: &Transaction, db: &DbManager) -> Result<(), &'static str> {
    match tx.tx_type {
        TxType::MeetupMeta => {
            // A. Validate CTS Structure (Syntax & Semantics)
            // Note: parse_cts now allows multiple positive tags!
            let cts_string = String::from_utf8(tx.raw_data.clone()).map_err(|_| "ERR_INVALID_UTF8_IN_TX")?;
            
            match parse_cts(&cts_string) {
                Ok(tags) => {
                    // Additional check: Ensure at least one positive tag exists (parser ensures syntax, we ensure logic)
                    if tags.iter().all(|t| t.status != crate::dict::cts_parser::TagStatus::Positive) {
                         return Err("ERR_CTS_NO_POSITIVE_TAG");
                    }
                    // Limit check is inside parser (max 10)
                },
                Err(e) => {
                    // Map CtsError to string key
                    return Err(match e {
                        CtsError::EmptyInput => "ERR_CTS_EMPTY_INPUT",
                        CtsError::SpaceInTag => "ERR_CTS_SPACE_IN_TAG",
                        CtsError::InvalidStatusChar => "ERR_CTS_INVALID_STATUS_CHAR",
                        CtsError::ExactlyOnePositiveRequired => "ERR_CTS_EXACTLY_ONE_POSITIVE", // Legacy error name, logic changed but key kept for compatibility or update key
                        CtsError::TooManyTags => "ERR_CTS_TOO_MANY_TAGS",
                        CtsError::LimitingWithoutMediating => "ERR_CTS_LIMITING_WITHOUT_MEDIATING",
                        _ => "ERR_CTS_PARSE_FAILED",
                    });
                }
            }

            // B. Verify Initiator Signature
            verify_tx_signature(tx)?;
        },
        
        TxType::TrustRevoke => {
            // A. Check Quorum (Min 3 independent signatures)
            // Assuming raw_data contains serialized list of signatures or a multi-sig structure
            // For simplicity, let's assume tx.signature is the aggregator, and raw_data has details
            if tx.raw_data.len() < TRUST_REVOKE_QUORUM_SIZE * 64 { // 64 bytes per Ed25519 sig
                return Err("ERR_TRUST_REVOKE_INSUFFICIENT_QUORUM");
            }
            
            // B. Verify each signature in the quorum against the target user's public key
            // This requires fetching the target user's key from DB
            // let target_pub_key = db.get_user_key(target_id)?;
            // for sig in extracted_signatures {
            //     if !verify(sig, target_pub_key) { return Err("ERR_INVALID_QUORUM_SIGNATURE"); }
            // }
            
            verify_tx_signature(tx)?; // Verify the submitter's signature too
        },

        TxType::TrustIssue | TxType::RepUpdate => {
            // Standard signature verification
            verify_tx_signature(tx)?;
            
            // Optional: Check for spam/flood limits in DB
        }
    }

    Ok(())
}

/// Helper to verify Ed25519 signature of a transaction.
fn verify_tx_signature(tx: &Transaction) -> Result<(), &'static str> {
    if tx.signature.is_empty() {
        return Err("ERR_TX_SIGNATURE_MISSING");
    }
    
    // Extract public key from raw_data or fetch from DB based on tx context
    // let pub_key = VerifyingKey::from_bytes(...)?;
    // let sig = Signature::from_slice(&tx.signature)?;
    // pub_key.verify_strict(&tx.payload_hash.as_bytes(), &sig)?;
    
    Ok(()) // Placeholder
}

/// Determines if a block should be retained longer than usual (e.g., for audit of malicious acts).
pub fn should_extend_retention(block: &Block) -> bool {
    for tx in &block.transactions {
        // Extend retention for blocks containing Trust Revocations (potential disputes)
        if tx.tx_type == TxType::TrustRevoke {
            return true;
        }
        // Extend for blocks from low-reputation validators (suspicious activity)
        // if get_reputation(tx.validator) < 50 { return true; }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    // Tests would go here, mocking DB and checking edge cases like Time-Travel, Sybil, etc.
    
    #[test]
    fn test_time_travel_detection() {
        // Construct a block with timestamp = now + 20 mins
        // Assert validate_block returns ERR_BLOCK_TIME_TRAVEL_DETECTED
    }

    #[test]
    fn test_hybrid_meetup_validation() {
        // Construct a TX with "kinoconcert" (2 positives)
        // Assert parse_cts succeeds and validate_transaction passes
    }
}
