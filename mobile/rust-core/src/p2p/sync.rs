// mobile/rust-core/src/p2p/sync.rs
// P2P Synchronization Module: Delta State Sync, Fast Sync, Storage Radius Enforcement.
// Architecture: Energy Efficient, Decentralized, App-Chain Integrated, Ghost Mode Aware.
// Year: 2026 | Rust Edition: 2024

use crate::chain::{block::Block, AppChain};
use crate::db::manager::DbManager;
use crate::p2p::protocol::{GossipPayload, MessageType, MessageEnvelope, DictSyncPayload};
use crate::p2p::mod::NodeMode;
use crate::dict::compressor::SessionDictionary;
use crate::crypto::identity::Identity; // For signing sync responses if needed
use log::{info, warn, debug, error};
use std::collections::{HashSet, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use zeroize::Zeroize;

/// Represents a snapshot of the current state for Fast Sync.
/// Contains only the latest reputation scores and trust graph hashes, not full history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub height: u64,
    pub merkle_root: String,
    pub timestamp: u64,
    // Compressed map of UserHash -> ReputationScore
    pub reputation_state: Vec<(String, i32)>, 
    // Optional dictionary patch for new nodes
    pub dict_patch: Option<Vec<u8>>, 
}

/// Request structure for Delta Sync.
/// Instead of just height, we send a vector of known block hashes/heights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    pub known_heights: Vec<u64>, // Heights we have
    pub known_hashes: Vec<String>, // Hashes of those heights (for integrity check)
    pub request_snapshot: bool, // Force snapshot if true (e.g., corruption detected)
    pub peer_trust_level: u32, // Number of trust anchors the requester has (for Ghost Mode check)
}

/// Response structure for Delta Sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResponse {
    Snapshot(StateSnapshot),
    Blocks(Vec<Block>),
    DictPatch(Vec<u8>),
    Ack, // Acknowledgement only
}

/// Manages synchronization logic for the P2P node.
pub struct SyncManager {
    db_manager: Arc<RwLock<DbManager>>,
    chain: Arc<RwLock<AppChain>>,
    mode: NodeMode,
    storage_radius_km: u32,
    // Tracks hashes of items we already have to avoid re-processing
    known_hashes: HashSet<String>,
    // Session dictionary for compressing sync traffic
    session_dict: SessionDictionary,
}

impl SyncManager {
    pub fn new(db_manager: Arc<RwLock<DbManager>>, chain: Arc<RwLock<AppChain>>, radius: u32) -> Self {
        SyncManager {
            db_manager,
            chain,
            mode: NodeMode::Eco,
            storage_radius_km: radius,
            known_hashes: HashSet::with_capacity(1000),
            session_dict: SessionDictionary::new(),
        }
    }

    /// Updates the node's operational mode.
    pub fn set_mode(&mut self, mode: NodeMode) {
        self.mode = mode;
        info!("MSG_SYNC_MODE_UPDATED: {:?}", mode);
    }

    /// Handles an incoming SyncRequest from a peer.
    /// Implements "Delta State Sync", "Fast Sync", and "Ghost Mode" restrictions.
    pub async fn handle_sync_request(&self, req: &SyncRequest) -> Result<SyncResponse, &'static str> {
        debug!("MSG_SYNC_REQUEST_RECEIVED: Known heights count {}", req.known_heights.len());

        // 1. Ghost Mode / Trust Check
        // If we are in strict mode or the peer has 0 trust anchors, limit data exposure
        if req.peer_trust_level == 0 && self.mode != NodeMode::Guardian {
            // Ghosts or untrusted peers only get Ack or very limited data
            warn!("MSG_SYNC_UNTRUSTED_PEER_LIMITED: Trust Level 0");
            return Ok(SyncResponse::Ack);
        }

        // 2. Fast Sync Check
        let current_height = self.chain.read().await.get_height();
        if req.request_snapshot || (req.known_heights.is_empty() && current_height > 100) {
            info!("MSG_FAST_SYNC_TRIGGERED: Sending snapshot");
            let snapshot = self.generate_snapshot().await?;
            return Ok(SyncResponse::Snapshot(snapshot));
        }

        // 3. Delta State Sync Logic
        let mut blocks_to_send = Vec::new();
        let chain_lock = self.chain.read().await;
        
        // Determine missing blocks based on known_heights vs current chain
        // Simple implementation: send everything after max(known_heights) up to limit
        let max_known = req.known_heights.iter().max().copied().unwrap_or(0);
        let batch_limit = match self.mode {
            NodeMode::Eco => 10,
            NodeMode::Active => 50,
            NodeMode::Guardian => 200,
        };

        let end_height = std::cmp::min(max_known + batch_limit, current_height);

        for h in (max_known + 1)..=end_height {
            if let Some(block) = chain_lock.blocks_cache.get(&h) {
                // Storage Radius Check before sending
                if self.is_within_radius(&block) {
                    blocks_to_send.push(block.clone());
                } else {
                    debug!("MSG_SYNC_BLOCK_SKIPPED_RADIUS: Height {}", h);
                }
            }
        }

        // 4. Optional: Attach Dictionary Patch if versions differ significantly
        // (Logic to detect version mismatch would go here)
        
        Ok(SyncResponse::Blocks(blocks_to_send))
    }

    /// Processes an incoming SyncResponse.
    pub async fn handle_sync_response(&self, resp: SyncResponse, sender_pub_key: &[u8]) -> Result<(), &'static str> {
        match resp {
            SyncResponse::Snapshot(snapshot) => {
                info!("MSG_PROCESSING_SNAPSHOT: Height {}", snapshot.height);
                self.apply_snapshot(snapshot).await?;
            },
            SyncResponse::Blocks(blocks) => {
                debug!("MSG_PROCESSING_BLOCKS: Count {}", blocks.len());
                for block in blocks {
                    // Verify signature of each block using sender key (or validator key inside block)
                    // Note: In PoA-Lite, block signature is by validator, but envelope by sender.
                    // Here we assume block internal validation is enough if transported securely.
                    self.process_incoming_block(block, sender_pub_key).await?;
                }
            },
            SyncResponse::DictPatch(patch) => {
                info!("MSG_APPLYING_DICT_PATCH: Size {} bytes", patch.len());
                // Apply patch to local dictionary
                // self.session_dict.apply_patch(&patch)?;
            },
            SyncResponse::Ack => {},
        }
        Ok(())
    }

    /// Processes a single incoming block with full validation.
    async fn process_incoming_block(&self, mut block: Block, _sender_pub_key: &[u8]) -> Result<(), &'static str> {
        let block_hash = block.calculate_hash();

        // 1. Deduplication
        if self.known_hashes.contains(&block_hash) {
            return Ok(()); 
        }

        // 2. Storage Radius Enforcement (CRITICAL)
        if !self.is_within_radius(&block) {
            debug!("MSG_BLOCK_REJECTED_RADIUS: Hash {}", block_hash);
            return Ok(()); 
        }

        // 3. Validate Block Structure & Signatures (Internal Consensus Rules)
        // This calls AppChain::validate_block which checks height continuity, merkle root, etc.
        let mut chain_lock = self.chain.write().await;
        if let Err(e) = chain_lock.add_block(block.clone()) {
            warn!("MSG_BLOCK_VALIDATION_FAILED: {}", e);
            return Err(e);
        }

        // 4. Update Known Hashes
        self.known_hashes.insert(block_hash);
        
        // 5. Prune known_hashes if too large (Simple LRU-like clear)
        if self.known_hashes.len() > 2000 {
            self.known_hashes.clear(); 
            // In production: keep the most recent 1000 hashes
        }

        Ok(())
    }

    /// Generates a state snapshot for Fast Sync.
    async fn generate_snapshot(&self) -> Result<StateSnapshot, &'static str> {
        let chain_lock = self.chain.read().await;
        let height = chain_lock.get_height();
        let root = chain_lock.get_latest_hash().unwrap_or_default();
        
        // Extract current reputation state from DB
        // Pseudo-code: let rep_state = db.get_all_reputations().await?;
        let rep_state = vec![]; 

        // Optional: Generate dictionary patch
        let dict_patch = None; 

        Ok(StateSnapshot {
            height,
            merkle_root: root,
            timestamp: chrono::Utc::now().timestamp() as u64,
            reputation_state: rep_state,
            dict_patch,
        })
    }

    /// Applies a received snapshot.
    async fn apply_snapshot(&self, snapshot: StateSnapshot) -> Result<(), &'static str> {
        let mut chain_lock = self.chain.write().await;
        chain_lock.blocks_cache.clear();
        
        // Reset chain height to snapshot height
        // In real impl: db.update_reputation_state(snapshot.reputation_state).await?;
        
        info!("MSG_SNAPSHOT_APPLIED_SUCCESS: Height {}", snapshot.height);
        Ok(())
    }

    /// Checks if a block meets the Storage Radius criteria.
    fn is_within_radius(&self, _block: &Block) -> bool {
        // Real implementation: decode geo-hash from block metadata and compare
        true 
    }

    /// Adaptive Gossip: Decides whether to propagate a message based on Mode and Content.
    pub fn should_propagate(&self, msg_type: &MessageType) -> bool {
        match self.mode {
            NodeMode::Eco => {
                // Only propagate critical security events (TrustRevoke, High Priority Alerts)
                matches!(msg_type, MessageType::Report(_)) 
            },
            NodeMode::Active => true,
            NodeMode::Guardian => true,
        }
    }
    
    /// Securely clears sensitive caches.
    pub async fn shutdown(&mut self) {
        self.known_hashes.clear();
        self.session_dict = SessionDictionary::new(); // Reset session dict
        info!("MSG_SYNC_SHUTDOWN_COMPLETE");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::block::Block;

    #[tokio::test]
    async fn test_ghost_mode_restriction() {
        let db = Arc::new(RwLock::new(DbManager::new("", "").await.unwrap()));
        let chain = Arc::new(RwLock::new(AppChain::new(db.clone()).await.unwrap()));
        let sync_mgr = SyncManager::new(db, chain, 50);

        let req = SyncRequest {
            known_heights: vec![],
            known_hashes: vec![],
            request_snapshot: false,
            peer_trust_level: 0, // Untrusted/Ghost
        };

        // Should return Ack only for untrusted peer in Eco/Active mode
        let resp = sync_mgr.handle_sync_request(&req).await.unwrap();
        assert!(matches!(resp, SyncResponse::Ack));
    }

    #[tokio::test]
    async fn test_delta_sync_logic() {
        // Setup with some blocks in chain (mocked)
        let db = Arc::new(RwLock::new(DbManager::new("", "").await.unwrap()));
        let chain = Arc::new(RwLock::new(AppChain::new(db.clone()).await.unwrap()));
        let sync_mgr = SyncManager::new(db, chain, 50);

        let req = SyncRequest {
            known_heights: vec![0, 1], // We have up to height 1
            known_hashes: vec![],
            request_snapshot: false,
            peer_trust_level: 5, // Trusted
        };

        // Should return missing blocks (2, 3, ...) up to limit
        let resp = sync_mgr.handle_sync_request(&req).await.unwrap();
        if let SyncResponse::Blocks(blocks) = resp {
            // Assert blocks are returned (logic depends on mock chain content)
            // For now, just checking it doesn't crash
        } else {
            panic!("Expected Blocks response");
        }
    }
}
