// mobile/rust-core/src/chain/mod.rs
// App-Chain Module: Lightweight distributed ledger for trust metadata ONLY.
// CRITICAL PRIVACY RULE: No raw phone numbers, no meetup content, no PII stored on-chain.
// Only Hashes (Phone Hash, Tx Hash) and Reputation Scores.
// Year: 2026 | Rust Edition: 2024

pub mod block;
pub mod transaction;
pub mod consensus;
pub mod merkle;

use crate::chain::block::{Block, Transaction};
use crate::db::manager::DbManager;
use crate::chain::transaction::TxType;
use log::{info, warn, error};
use std::collections::HashMap;

/// Main structure managing the local copy of the App-Chain.
pub struct AppChain {
    blocks_cache: HashMap<u64, Block>,
    current_height: u64,
    db_manager: DbManager,
}

impl AppChain {
    pub async fn new(db_manager: DbManager) -> Result<Self, &'static str> {
        info!("MSG_APPCHAIN_INIT_START");
        // Load state from encrypted SQLCipher DB
        let initial_height = 0; // Placeholder for DB load
        
        Ok(AppChain {
            blocks_cache: HashMap::new(),
            current_height: initial_height,
            db_manager,
        })
    }

    /// Adds a block after strict validation.
    pub fn add_block(&mut self, block: Block) -> Result<(), &'static str> {
        // 1. Height Continuity
        if block.header.height != self.current_height + 1 {
            return Err("ERR_INVALID_BLOCK_HEIGHT");
        }

        // 2. Consensus: Validator must have RepScore > 80
        if !consensus::validate_validator_reputation(&block.header.validator_id, &self.db_manager) {
            return Err("ERR_VALIDATOR_LOW_REPUTATION");
        }

        // 3. Cryptographic Verification (Signatures & Merkle Root)
        if !block.verify_integrity() {
            return Err("ERR_BLOCK_INTEGRITY_CHECK_FAILED");
        }

        // 4. Commit
        self.blocks_cache.insert(block.header.height, block.clone());
        self.current_height = block.header.height;
        
        // Trigger async DB save in background (not shown here)
        info!("MSG_BLOCK_ADDED_HEIGHT_{}", self.current_height);
        Ok(())
    }

    /// Adaptive Pruning Mechanism.
    /// RULE: Low reputation users' data is kept LONGER (up to 1 year) for audit/fraud proof.
    /// High reputation users' data is pruned sooner (e.g., 30 days) to save space.
    pub async fn adaptive_prune(&mut self) -> Result<(), &'static str> {
        info!("MSG_APPCHAIN_ADAPTIVE_PRUNE_START");
        
        let now = chrono::Utc::now().timestamp();
        
        // Iterate through blocks and decide retention based on involved users' reputation
        // Pseudo-logic:
        // for block in self.blocks_cache.values() {
        //    let min_rep = get_min_reputation_involved(block);
        //    let retention_secs = if min_rep < 20 { ONE_YEAR } else { THIRTY_DAYS };
        //    if now - block.header.timestamp > retention_secs { remove(block); }
        // }
        
        self.db_manager.perform_adaptive_prune(now).await?;
        info!("MSG_APPCHAIN_PRUNE_COMPLETE");
        Ok(())
    }
}
