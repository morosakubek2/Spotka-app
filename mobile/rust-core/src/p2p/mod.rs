// mobile/rust-core/src/p2p/mod.rs
// P2P Network Module: Hybrid Transport (TCP/QUIC + BLE), Energy Efficient, Adaptive.
// Features: Storage Radius Filtering, Geo-fenced BLE, Guardian Mode, Offline Fallback.
// Year: 2026 | Rust Edition: 2024

pub mod transport;
pub mod discovery;
pub mod protocol;
pub mod sync;

use crate::crypto::identity::Identity;
use crate::db::manager::DbManager;
use crate::db::consts::DEFAULT_STORAGE_RADIUS_KM;
use libp2p::{identity, PeerId};
use log::{info, warn, error};
use std::sync::Arc;
use tokio::sync::RwLock;
use zeroize::Zeroize; // For secure memory wiping

/// Operational modes for the P2P node to manage energy consumption.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeMode {
    /// Eco-Node: Minimal activity, sleeps often, only responds to direct pings.
    Eco,
    /// Active-Node: Standard operation, participates in gossip.
    Active,
    /// Network-Guardian: High activity, relays traffic, frequent syncs.
    Guardian,
}

/// Metrics for the DevTools panel (Live monitoring).
#[derive(Debug, Clone, Default)]
pub struct NodeMetrics {
    pub connected_peers: u32,
    pub inbound_traffic_bytes: u64,
    pub outbound_traffic_bytes: u64,
    pub last_sync_timestamp: u64,
    pub active_mode_duration_secs: u64,
}

/// Main structure representing the P2P Node.
pub struct P2PNode {
    pub peer_id: PeerId,
    pub identity: Identity,
    pub mode: NodeMode,
    pub storage_radius_km: u32, // Dynamic radius from settings
    pub db_manager: Arc<RwLock<DbManager>>,
    pub metrics: Arc<RwLock<NodeMetrics>>,
    // swarm: Swarm<Behaviour>, // Hidden in production code
}

impl P2PNode {
    /// Initializes the P2P Node.
    pub async fn new(identity: Identity, db_manager: DbManager) -> Result<Self, &'static str> {
        info!("MSG_P2P_NODE_INIT_START");

        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        // Load storage radius from DB config (default 60km)
        // let storage_radius = db_manager.get_config("storage_radius").unwrap_or(DEFAULT_STORAGE_RADIUS_KM);
        let storage_radius = DEFAULT_STORAGE_RADIUS_KM; 

        let node = P2PNode {
            peer_id,
            identity,
            mode: NodeMode::Eco,
            storage_radius_km: storage_radius,
            db_manager: Arc::new(RwLock::new(db_manager)),
            metrics: Arc::new(RwLock::new(NodeMetrics::default())),
        };

        info!("MSG_P2P_NODE_READY: PeerID {}", peer_id);
        Ok(node)
    }

    /// Updates node mode based on battery/power.
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

    /// Updates the storage radius dynamically from settings.
    pub fn set_storage_radius(&mut self, radius_km: u32) {
        self.storage_radius_km = radius_km;
        info!("MSG_P2P_STORAGE_RADIUS_UPDATED: {} km", radius_km);
        // Trigger pruning of data outside new radius
    }

    /// Critical Filter: Checks if a packet's location hash is within storage radius.
    /// Returns true if packet should be processed, false if it should be dropped immediately.
    pub fn is_within_storage_radius(&self, packet_location_hash: &str, user_location_hash: &str) -> bool {
        // In production: Decode hashes to coords, calculate Haversine distance.
        // If distance > self.storage_radius_km -> return false.
        
        // Placeholder logic for demonstration
        let distance_km = self.calculate_distance_mock(packet_location_hash, user_location_hash);
        if distance_km > self.storage_radius_km as f32 {
            warn!("MSG_P2P_PACKET_DROPPED_OUT_OF_RADIUS: {} km", distance_km);
            return false;
        }
        true
    }

    /// Mock distance calculator (replace with real geo logic)
    fn calculate_distance_mock(&self, _hash1: &str, _hash2: &str) -> f32 {
        0.0 // Assume within radius for now
    }

    /// Determines if BLE should be active (Geo-fencing).
    pub fn should_enable_ble(&self, distance_to_meetup_km: Option<f32>) -> bool {
        if self.mode == NodeMode::Eco {
            return false;
        }
        match distance_to_meetup_km {
            Some(dist) => dist <= 1.0, // Enable only within 1km
            None => false,
        }
    }

    /// Pre-processes incoming payload: Checks radius BEFORE decryption to save CPU.
    pub async fn filter_and_process_packet(&self, packet: &crate::p2p::protocol::Packet) -> Result<(), &'static str> {
        // 1. Check Storage Radius (Cheap operation)
        if !self.is_within_storage_radius(&packet.location_hash, &self.get_user_location_hash()) {
            return Err("ERR_PACKET_OUT_OF_RADIUS");
        }

        // 2. Decrypt and Process (Expensive operation)
        // ... decryption logic ...

        Ok(())
    }

    fn get_user_location_hash(&self) -> String {
        // Retrieve current user location hash from DB/Sensors
        "user_loc_hash".to_string()
    }

    /// Runs the main event loop.
    pub async fn run(&self) -> Result<(), &'static str> {
        info!("MSG_P2P_RUN_LOOP_START");
        // Loop implementation
        Ok(())
    }

    /// Securely shuts down the node, wiping sensitive session data.
    pub async fn shutdown(&mut self) {
        info!("MSG_P2P_SHUTDOWN_START");
        
        // Wipe metrics or sensitive caches if needed
        let mut m = self.metrics.write().await;
        m.inbound_traffic_bytes.zeroize();
        m.outbound_traffic_bytes.zeroize();
        drop(m);

        info!("MSG_P2P_SHUTDOWN_COMPLETE");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_radius_filter() {
        let mut node = P2PNode {
            peer_id: PeerId::random(),
            identity: Identity::generate("123"),
            mode: NodeMode::Active,
            storage_radius_km: 60,
            db_manager: Arc::new(RwLock::new(DbManager::new("", "").await.unwrap())),
            metrics: Arc::new(RwLock::new(NodeMetrics::default())),
        };

        // Simulate packet from 70km away
        assert!(!node.is_within_storage_radius("far_hash", "user_hash"));
        
        // Change radius to 100km
        node.set_storage_radius(100);
        // Now it should pass (assuming mock distance is constant or logic updated)
        // Note: Mock returns 0.0, so it passes. Real logic would fail the first check.
    }
}
