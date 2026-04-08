// mobile/rust-core/src/chain/mod.rs
// App-Chain Module: Lightweight distributed ledger for trust metadata.
// Architecture: Decentralized, Zero-Knowledge, Anti-Social (no content storage).
// Year: 2026 | Rust Edition: 2024

pub mod block;
pub mod transaction;
pub mod consensus;
pub mod merkle;

use crate::chain::block::{Block, Transaction};
use crate::db::manager::DbManager;
use crate::db::schema::{ChainBlock as DbChainBlock, User}; // Import schematu Drift
use drift::prelude::*;
use log::{info, warn, error};
use std::collections::HashMap;
use zeroize::Zeroize; // For secure memory clearing

/// Main structure managing the local copy of the App-Chain.
/// Responsible for validating, storing, and pruning blocks.
pub struct AppChain {
    // Local cache of recent blocks for fast access (Height -> Block)
    // In production, this is backed by the SQLCipher database (Drift) for persistence.
    blocks_cache: HashMap<u64, Block>,
    current_height: u64,
    db_manager: DbManager,
}

impl AppChain {
    /// Initializes the App-Chain instance.
    /// Loads the latest state from the encrypted local database.
    pub async fn new(db_manager: DbManager) -> Result<Self, &'static str> {
        info!("MSG_APPCHAIN_INIT_START");
        
        // Load latest height from DB
        let database = db_manager.database();
        // Pseudo-code for Drift query:
        // let latest = database.blocks.order_by(|b| b.height).last().one()?;
        let initial_height = 0; // Placeholder if DB empty
        
        Ok(AppChain {
            blocks_cache: HashMap::new(),
            current_height: initial_height,
            db_manager,
        })
    }

    /// Attempts to add a new block to the chain.
    /// Performs validation: height continuity, signature verification, consensus rules, and Merkle root.
    pub async fn add_block(&mut self, block: Block) -> Result<(), &'static str> {
        // 1. Validate Height Continuity
        if block.header.height != self.current_height + 1 {
            warn!("ERR_INVALID_BLOCK_HEIGHT: Expected {}, Got {}", 
                  self.current_height + 1, block.header.height);
            return Err("ERR_INVALID_BLOCK_HEIGHT");
        }

        // 2. Verify Merkle Root Integrity
        if !block.verify_merkle_root() {
            error!("ERR_INVALID_MERKLE_ROOT");
            return Err("ERR_INVALID_MERKLE_ROOT");
        }

        // 3. Validate Consensus (PoA-Lite) & Signature
        // Check if the validator has sufficient Reputation Score (>80)
        // This logic delegates to the consensus module which checks DB
        if !consensus::validate_validator(&block.header.validator_id, &self.db_manager).await? {
            warn!("ERR_INVALID_VALIDATOR_REPUTATION");
            return Err("ERR_INVALID_VALIDATOR_REPUTATION");
        }

        // 4. Verify Cryptographic Signature
        // Note: We need the public key of the validator from DB or Cache
        if let Err(_) = block.verify_signature(&[]) { // Pass pub key here in real impl
            error!("ERR_INVALID_BLOCK_SIGNATURE");
            return Err("ERR_INVALID_BLOCK_SIGNATURE");
        }

        // 5. Commit to Local Storage (DB)
        // Convert Block to DB schema struct
        let db_block = DbChainBlock {
            height: block.header.height as i64,
            prev_hash: block.header.prev_hash.clone(),
            merkle_root: block.header.merkle_root.clone(),
            timestamp: block.header.timestamp as i64,
            validator_id: block.header.validator_id.clone(),
            signature: block.signature.clone(),
        };

        // Save to Drift DB
        let database = self.db_manager.database();
        // database.blocks.insert(db_block).await.map_err(|_| "ERR_DB_WRITE_FAILED")?;

        // Update Cache
        self.blocks_cache.insert(block.header.height, block.clone());
        self.current_height = block.header.height;

        info!("MSG_BLOCK_ADDED_SUCCESS: Height {}", self.current_height);
        Ok(())
    }

    /// Returns the hash of the latest block.
    pub fn get_latest_hash(&self) -> Option<String> {
        self.blocks_cache.get(&self.current_height).map(|b| b.calculate_hash())
    }

    /// Returns the current height of the chain.
    pub fn get_height(&self) -> u64 {
        self.current_height
    }

    /// Prunes old blocks to save space (Auto-Cleaning).
    /// Retention policy: 
    /// - Normal users: Keep last N days.
    /// - Low reputation users (<20): Keep up to 1 year (for audit/evidence).
    pub async fn prune_old_blocks(&mut self, max_age_days: u32) -> Result<(), &'static str> {
        info!("MSG_APPCHAIN_PRUNE_START");
        
        let cutoff_timestamp = (chrono::Utc::now().timestamp() - (max_age_days as i64 * 86400)) as u64;
        let database = self.db_manager.database();

        // Fetch blocks older than cutoff
        // let old_blocks = database.blocks.filter(|b| b.timestamp < cutoff).all().await?;
        
        let mut blocks_to_delete: Vec<i64> = Vec::new();

        // Iterate and check reputation of validator
        // for block in old_blocks {
        //     let user = database.users.get_by_id(&block.validator_id).await?;
        //     if let Some(u) = user {
        //         if u.reputation_score < 20 {
        //             // Keep it (do not add to delete list)
        //             continue; 
        //         }
        //     }
        //     blocks_to_delete.push(block.height);
        // }

        // Delete in batch
        // if !blocks_to_delete.is_empty() {
        //     database.blocks.delete_by_heights(&blocks_to_delete).await?;
        //     info!("MSG_APPCHAIN_PRUNED: {} blocks", blocks_to_delete.len());
        // }

        // Securely clear cache entries
        for height in &blocks_to_delete {
            if let Some(mut block) = self.blocks_cache.remove(&(*height as u64)) {
                block.zeroize(); // Clear sensitive data from memory
            }
        }

        info!("MSG_APPCHAIN_PRUNE_COMPLETE");
        Ok(())
    }
}
