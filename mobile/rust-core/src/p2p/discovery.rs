// mobile/rust-core/src/p2p/discovery.rs
// Hybrid Node Discovery: mDNS (WiFi), BLE (Local), Signaling (Premium).
// Features: Geofencing, Storage Radius Filtering, "Ghost" Mode, Adaptive Scanning.
// Year: 2026 | Rust Edition: 2024

use crate::p2p::mod::NodeMode;
use crate::db::manager::DbManager;
use libp2p::{mdns, PeerId, Multiaddr};
use log::{info, warn, debug};
use std::time::Duration;

/// Configuration for discovery behavior based on energy constraints.
pub struct DiscoveryConfig {
    pub mode: NodeMode,
    pub storage_radius_km: u32,
    pub has_trust_certificates: bool, // False = "Ghost" mode
}

impl DiscoveryConfig {
    /// Returns the scanning interval based on the node mode.
    /// Eco nodes scan rarely to save battery. Guardians scan continuously.
    pub fn get_scan_interval(&self) -> Duration {
        match self.mode {
            NodeMode::Eco => Duration::from_secs(300), // 5 minutes
            NodeMode::Active => Duration::from_secs(60), // 1 minute
            NodeMode::Guardian => Duration::from_secs(10), // 10 seconds
        }
    }

    /// Returns true if BLE advertising/scanning should be enabled.
    /// BLE is energy-intensive, so it's restricted by mode and proximity.
    pub fn should_enable_ble(&self, near_meetup: bool) -> bool {
        if !self.has_trust_certificates {
            return false; // Ghosts don't advertise
        }
        match self.mode {
            NodeMode::Eco => near_meetup, // Only wake up BLE if very close to a meetup
            NodeMode::Active | NodeMode::Guardian => true,
        }
    }
}

/// Handles the discovery of peers via mDNS and BLE.
pub struct DiscoveryService {
    config: DiscoveryConfig,
    // mdns_behavior: mdns::tokio::Behaviour,
    // ble_scanner: BleScanner, // Placeholder for native BLE bridge
}

impl DiscoveryService {
    pub fn new(config: DiscoveryConfig) -> Self {
        info!("MSG_DISCOVERY_INIT: Mode={:?}, Radius={}km", config.mode, config.storage_radius_km);
        
        // Check "Ghost" condition
        if !config.has_trust_certificates {
            warn!("MSG_GHOST_MODE_ACTIVE: No trust certificates. Hidden from global discovery.");
        }

        DiscoveryService {
            config,
            // mdns_behavior: mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id),
        }
    }

    /// Updates the configuration dynamically (e.g., when user changes Storage Radius).
    pub fn update_config(&mut self, new_config: DiscoveryConfig) {
        let changed = self.config.storage_radius_km != new_config.storage_radius_km;
        self.config = new_config;
        if changed {
            info!("MSG_STORAGE_RADIUS_UPDATED: {}km", self.config.storage_radius_km);
            // Trigger re-evaluation of known peers against new radius
        }
    }

    /// Filters a discovered peer based on the Storage Radius rule.
    /// Returns true if the peer should be kept/connected, false if rejected.
    pub fn filter_by_radius(&self, peer_distance_km: f32) -> bool {
        if peer_distance_km <= self.config.storage_radius_km as f32 {
            true
        } else {
            debug!("MSG_PEER_REJECTED_RADIUS: Distance {} > Limit {}", peer_distance_km, self.config.storage_radius_km);
            false
        }
    }

    /// Handles a discovered peer event (mDNS or BLE).
    /// Performs validation before passing to the Swarm.
    pub fn on_peer_discovered(&self, peer_id: &PeerId, address: &Multiaddr, distance_km: f32) -> Option<Multiaddr> {
        // 1. Ghost Mode Check: If we are a ghost, we might still listen but not connect broadly?
        // Actually, Ghosts can connect outbound, but won't be found inbound easily.
        
        // 2. Storage Radius Check (Critical for P2P efficiency)
        if !self.filter_by_radius(distance_km) {
            return None; // Drop packet/connection attempt early
        }

        // 3. Trust Check (Optional: Only connect to verified users in strict modes?)
        // For now, allow connection to establish trust.

        info!("MSG_PEER_VALIDATED: {} at {:.2}km", peer_id, distance_km);
        Some(address.clone())
    }

    /// Generates BLE Advertising Payload.
    /// Contains only essential info to minimize airtime: PeerID prefix + Service UUID.
    pub fn generate_ble_payload(&self) -> Vec<u8> {
        if !self.config.should_enable_ble(false) {
            return vec![];
        }
        // Construct minimal payload (e.g., first 8 bytes of PeerID + Spotka Service UUID)
        // This is picked up by other devices running Spotka nearby.
        vec![0x01, 0x02, 0x03] // Placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radius_filtering() {
        let config = DiscoveryConfig {
            mode: NodeMode::Active,
            storage_radius_km: 50,
            has_trust_certificates: true,
        };
        let service = DiscoveryService::new(config);

        assert!(service.filter_by_radius(10.0)); // Inside
        assert!(service.filter_by_radius(50.0)); // Edge
        assert!(!service.filter_by_radius(51.0)); // Outside
    }

    #[test]
    fn test_ghost_mode_ble() {
        let config = DiscoveryConfig {
            mode: NodeMode::Guardian,
            storage_radius_km: 100,
            has_trust_certificates: false, // Ghost
        };
        let service = DiscoveryService::new(config);
        
        // Even as Guardian, Ghost shouldn't advertise BLE
        assert!(!service.config.should_enable_ble(true));
    }
}
