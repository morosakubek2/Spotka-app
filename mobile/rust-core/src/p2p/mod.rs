// mobile/rust-core/src/p2p/mod.rs
// P2P Network Module: Private Mesh Architecture (State & Configuration).
// Architecture: 
//   - Behaviour Layer (behaviour.rs): Handles event loop, filtering, and routing logic.
//   - Node Layer (this file): Manages State (TrustGraph), Metrics, and Configuration.
// Features: 
//   - No Public Gossip (Free Tier).
//   - Chain-of-Trust for Invites.
//   - "Friends Only" Mode Enforcement.
// Security: Zero-Knowledge, Memory Safe (Zeroize).
// Year: 2026 | Rust Edition: 2024

pub mod transport;
pub mod discovery;
pub mod protocol;
pub mod sync;
pub mod behaviour; // NEW: Central event handler

use crate::crypto::identity::Identity;
use crate::db::manager::DbManager;
use crate::db::consts::DEFAULT_STORAGE_RADIUS_KM;
use libp2p::{identity, PeerId};
use log::{info, warn, debug};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use zeroize::{Zeroize, Zeroizing};

/// Operational modes for the P2P node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeMode {
    Eco,      // Minimal activity, direct messages only.
    Active,   // Standard operation, forwards invites for friends.
    Guardian, // High activity, helps relay network traffic for trusted subnets.
}

/// Relationship type in our Private Mesh.
#[derive(Debug, Clone, PartialEq)]
pub enum TrustLevel {
    /// Physically verified via Ping (Full Trust).
    VerifiedFriend,
    /// Pending verification (Invited Guest). Limited access.
    InvitedGuest, 
    /// No trust.
    None,
}

/// The Trust Graph: Maps PeerID <-> UserHash.
/// Acts as a fast cache backed by the local DB.
pub struct TrustGraph {
    peers: HashMap<PeerId, (String, TrustLevel)>,
    user_to_peer: HashMap<String, PeerId>,
}

impl TrustGraph {
    pub fn new() -> Self {
        TrustGraph {
            peers: HashMap::new(),
            user_to_peer: HashMap::new(),
        }
    }

    pub fn add_verified(&mut self, peer_id: PeerId, user_hash: String) {
        self.peers.insert(peer_id, (user_hash.clone(), TrustLevel::VerifiedFriend));
        self.user_to_peer.insert(user_hash, peer_id);
        info!("MSG_TRUST_GRAPH_VERIFIED: {}", user_hash);
    }

    pub fn add_guest(&mut self, peer_id: PeerId, user_hash: String) {
        self.peers.insert(peer_id, (user_hash.clone(), TrustLevel::InvitedGuest));
        self.user_to_peer.insert(user_hash, peer_id);
        info!("MSG_TRUST_GRAPH_GUEST_ADDED: {}", user_hash);
    }

    pub fn get_level(&self, peer_id: &PeerId) -> TrustLevel {
        self.peers.get(peer_id).map(|(_, level)| level.clone()).unwrap_or(TrustLevel::None)
    }

    pub fn get_peer_for_user(&self, user_hash: &str) -> Option<PeerId> {
        self.user_to_peer.get(user_hash).copied()
    }

    pub fn get_user_hash_for_peer(&self, peer_id: &PeerId) -> Option<String> {
        self.peers.get(peer_id).map(|(hash, _)| hash.clone())
    }
    
    pub fn remove_guest(&mut self, peer_id: &PeerId) {
        if let Some((user_hash, _)) = self.peers.remove(peer_id) {
            self.user_to_peer.remove(&user_hash);
            info!("MSG_TRUST_GRAPH_GUEST_REMOVED: {}", user_hash);
        }
    }
}

impl Zeroize for TrustGraph {
    fn zeroize(&mut self) {
        self.peers.zeroize();
        self.peers.clear();
        self.user_to_peer.zeroize();
        self.user_to_peer.clear();
    }
}

/// Metrics for monitoring.
#[derive(Debug, Clone, Default)]
pub struct NodeMetrics {
    pub connected_peers: u32,
    pub inbound_traffic_bytes: u64,
    pub outbound_traffic_bytes: u64,
    pub packets_dropped_radius: u32,
    pub packets_dropped_trust: u32,
    pub packets_dropped_capacity: u32,
    pub packets_dropped_friends_only: u32,
    pub invites_forwarded: u32,
}

/// Main structure representing the P2P Node (State Manager).
/// Note: Packet filtering logic is now in `behaviour::SpotkaBehaviour`.
pub struct P2PNode {
    pub peer_id: PeerId,
    pub identity: Identity,
    pub mode: NodeMode,
    pub storage_radius_km: u32,
    pub db_manager: Arc<RwLock<DbManager>>,
    pub metrics: Arc<RwLock<NodeMetrics>>,
    pub is_ghost_mode: bool,
    pub trust_graph: Arc<RwLock<TrustGraph>>,
}

impl P2PNode {
    pub async fn new(identity: Identity, db_manager: DbManager) -> Result<Self, &'static str> {
        info!("MSG_P2P_NODE_INIT_START (Private Mesh Mode)");

        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());
        let storage_radius = DEFAULT_STORAGE_RADIUS_KM; 

        let node = P2PNode {
            peer_id,
            identity,
            mode: NodeMode::Eco,
            storage_radius_km: storage_radius,
            db_manager: Arc::new(RwLock::new(db_manager)),
            metrics: Arc::new(RwLock::new(NodeMetrics::default())),
            is_ghost_mode: false,
            trust_graph: Arc::new(RwLock::new(TrustGraph::new())),
        };

        info!("MSG_P2P_NODE_READY: PeerID {}", peer_id);
        Ok(node)
    }

    // --- Trust Management (Called by Behaviour) ---

    pub async fn on_ping_success(&self, peer_id: PeerId, user_hash: String) {
        let mut graph = self.trust_graph.write().await;
        graph.add_verified(peer_id, user_hash);
    }

    pub async fn on_invite_accepted(&self, peer_id: PeerId, user_hash: String) {
        let mut graph = self.trust_graph.write().await;
        graph.add_guest(peer_id, user_hash);
    }

    // --- Forwarding Logic (Called by Behaviour) ---

    /// Checks if forwarding is allowed based on "Friends Only" flag.
    /// Returns target PeerID if valid.
    pub async fn resolve_forward_target(
        &self, 
        target_user_hash: &str, 
        is_friends_only: bool,
        sender_is_organizer: bool
    ) -> Result<PeerId, &'static str> {
        
        if is_friends_only && !sender_is_organizer {
            let mut m = self.metrics.write().await;
            m.packets_dropped_friends_only += 1;
            warn!("MSG_FORWARD_BLOCKED_FRIENDS_ONLY: Target {}", target_user_hash);
            return Err("ERR_FORWARDING_RESTRICTED");
        }

        let graph = self.trust_graph.read().await;
        
        if let Some(target_peer) = graph.get_peer_for_user(target_user_hash) {
            Ok(target_peer)
        } else {
            Err("ERR_TARGET_USER_NOT_IN_MESH")
        }
    }

    pub fn update_mode(&mut self, battery_level: u8, is_charging: bool, _is_wifi: bool) {
        let old_mode = self.mode;
        self.mode = if is_charging && battery_level > 80 {
            NodeMode::Guardian
        } else if battery_level < 60 && !is_charging {
            NodeMode::Eco
        } else {
            NodeMode::Active
        };

        if old_mode != self.mode {
            info!("MSG_P2P_MODE_CHANGED: {:?} -> {:?}", old_mode, self.mode);
        }
    }

    pub async fn shutdown(&mut self) {
        info!("MSG_P2P_SHUTDOWN_START");
        
        let mut m = self.metrics.write().await;
        m.inbound_traffic_bytes.zeroize();
        m.outbound_traffic_bytes.zeroize();
        drop(m);

        let mut graph = self.trust_graph.write().await;
        graph.zeroize();
        drop(graph);

        info!("MSG_P2P_SHUTDOWN_COMPLETE");
    }
}

// Re-export types for convenience
pub use protocol::{MessageEnvelope, MessageType};
pub use behaviour::SpotkaBehaviour;
