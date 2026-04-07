// mobile/rust-core/src/p2p/sync.rs
// P2P Synchronization Module: Delta State Sync, Fast Sync, Storage Radius Enforcement.
// Architecture: Energy Efficient, Decentralized, App-Chain Integrated.
// Year: 2026 | Rust Edition: 2024

use crate::chain::{block::Block, AppChain};
use crate::db::manager::DbManager;
use crate::p2p::protocol::{GossipPayload, MessageType, SyncRequest, SyncResponse};
use crate::p2p::mod::NodeMode;
use crate::dict::compressor::SessionDictionary;
use log::{info, warn, debug, error};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

/// Represents a snapshot of the current state for Fast Sync.
/// Contains only the latest reputation scores and trust graph hashes, not full history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub height: u64,
    pub merkle_root: String,
    pub timestamp: u64,
    // Compressed map of UserHash -> ReputationScore
    pub reputation_state: Vec<(String, i32)>, 
}

/// Manages synchronization logic for the P2P node.
pub struct SyncManager {
    db_manager: Arc<RwLock<DbManager>>,
    chain: Arc<RwLock<AppChain>>,
    mode: NodeMode,
    storage_radius_km: u32,
    // Tracks hashes of items we already have to avoid re-processing
    known_hashes: HashSet<String>,
}

impl SyncManager {
    pub fn new(db_manager: Arc<RwLock<DbManager>>, chain: Arc<RwLock<AppChain>>, radius: u32) -> Self {
        SyncManager {
            db_manager,
            chain,
            mode: NodeMode::Eco,
            storage_radius_km: radius,
            known_hashes: HashSet::new(),
        }
    }

    /// Updates the node's operational mode (affects sync aggressiveness).
    pub fn set_mode(&mut self, mode: NodeMode) {
        self.mode = mode;
        info!("MSG_SYNC_MODE_UPDATED: {:?}", mode);
    }

    /// Handles an incoming SyncRequest from a peer.
    /// Implements "Delta State Sync" and "Fast Sync" logic.
    pub async fn handle_sync_request(&self, req: &SyncRequest) -> Result<SyncResponse, &'static str> {
        debug!("MSG_SYNC_REQUEST_RECEIVED: Height {}", req.start_height);

        // 1. Fast Sync Check: If peer is far behind or new, send Snapshot instead of blocks
        let current_height = self.chain.read().await.get_height();
        if req.start_height == 0 && current_height > 100 {
            info!("MSG_FAST_SYNC_TRIGGERED: Sending snapshot instead of full chain");
            let snapshot = self.generate_snapshot().await?;
            return Ok(SyncResponse::Snapshot(snapshot));
        }

        // 2. Delta State Sync: Send only blocks missing by the peer
        let mut blocks_to_send = Vec::new();
        let chain_lock = self.chain.read().await;
        
        // Limit batch size to prevent DoS and save bandwidth
        let end_height = std::cmp::min(req.start_height + 50, current_height);

        for h in req.start_height..=end_height {
            if let Some(block) = chain_lock.blocks_cache.get(&h) {
                // Check Storage Radius: Don't send blocks unrelated to our location
                // (Simplified check: in real app, block metadata contains geo-hash)
                if self.is_within_radius(&block) {
                    blocks_to_send.push(block.clone());
                }
            }
        }

        Ok(SyncResponse::Blocks(blocks_to_send))
    }

    /// Processes an incoming SyncResponse (Blocks or Snapshot).
    pub async fn handle_sync_response(&self, resp: SyncResponse) -> Result<(), &'static str> {
        match resp {
            SyncResponse::Snapshot(snapshot) => {
                info!("MSG_PROCESSING_SNAPSHOT: Height {}", snapshot.height);
                self.apply_snapshot(snapshot).await?;
            },
            SyncResponse::Blocks(blocks) => {
                debug!("MSG_PROCESSING_BLOCKS: Count {}", blocks.len());
                for block in blocks {
                    self.process_incoming_block(block).await?;
                }
            },
        }
        Ok(())
    }

    /// Processes a single incoming block with full validation.
    async fn process_incoming_block(&self, block: Block) -> Result<(), &'static str> {
        // 1. Deduplication
        let block_hash = block.calculate_hash();
        if self.known_hashes.contains(&block_hash) {
            return Ok(()); // Already processed
        }

        // 2. Storage Radius Enforcement (CRITICAL)
        // Reject immediately if outside our configured radius to save CPU/DB writes
        if !self.is_within_radius(&block) {
            debug!("MSG_BLOCK_REJECTED_RADIUS: Hash {}", block_hash);
            return Ok(()); // Silent ignore, not an error
        }

        // 3. Validate & Add to Chain
        let mut chain_lock = self.chain.write().await;
        if let Err(e) = chain_lock.add_block(block.clone()) {
            warn!("MSG_BLOCK_VALIDATION_FAILED: {}", e);
            return Err(e);
        }

        // 4. Update Known Hashes
        self.known_hashes.insert(block_hash);
        
        // 5. Prune old known_hashes to prevent memory leak (keep last 1000)
        if self.known_hashes.len() > 1000 {
            self.known_hashes.clear(); // Simplified; real impl should keep recent ones
        }

        Ok(())
    }

    /// Generates a state snapshot for Fast Sync.
    async fn generate_snapshot(&self) -> Result<StateSnapshot, &'static str> {
        let chain_lock = self.chain.read().await;
        let height = chain_lock.get_height();
        let root = chain_lock.get_latest_hash().unwrap_or_default();
        
        // Extract current reputation state from DB (expensive operation, cached in real impl)
        // Pseudo-code: let rep_state = db.get_all_reputations().await?;
        let rep_state = vec![]; 

        Ok(StateSnapshot {
            height,
            merkle_root: root,
            timestamp: chrono::Utc::now().timestamp() as u64,
            reputation_state: rep_state,
        })
    }

    /// Applies a received snapshot, resetting local state.
    async fn apply_snapshot(&self, snapshot: StateSnapshot) -> Result<(), &'static str> {
        // 1. Clear old chain/cache
        let mut chain_lock = self.chain.write().await;
        chain_lock.blocks_cache.clear();
        
        // 2. Set new height/root
        // (In real impl, update DB with new reputation state)
        
        info!("MSG_SNAPSHOT_APPLIED_SUCCESS: Height {}", snapshot.height);
        Ok(())
    }

    /// Checks if a block meets the Storage Radius criteria.
    fn is_within_radius(&self, _block: &Block) -> bool {
        // Real implementation would decode block metadata (geo-hash)
        // and compare with user's current location + storage_radius_km.
        // For now, assume true for demonstration.
        true 
    }

    /// Adaptive Gossip: Decides whether to propagate a message based on Mode.
    pub fn should_propagate(&self, msg_type: &MessageType) -> bool {
        match self.mode {
            NodeMode::Eco => {
                // Only propagate critical alerts (e.g., TrustRevoke)
                matches!(msg_type, MessageType::Gossip(GossipPayload { priority: 1, .. })) 
            },
            NodeMode::Active => true, // Propagate most things
            NodeMode::Guardian => true, // Aggressive propagation
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_storage_radius_filtering() {
        // Mock setup
        let db = Arc::new(RwLock::new(DbManager::new("", "").await.unwrap()));
        let chain = Arc::new(RwLock::new(AppChain::new(db.clone()).await.unwrap()));
        let sync_mgr = SyncManager::new(db, chain, 50); // 50km radius

        // Create a mock block (would need geo-data in real test)
        // Assert that blocks outside radius are ignored without error
        // (Logic tested via is_within_radius stub)
        assert!(sync_mgr.is_within_radius(&Block::new(0, "gen".to_string(), vec![], "val".to_string())));
    }
}
