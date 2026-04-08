// mobile/rust-core/src/p2p/discovery.rs
// Hybrid Node Discovery: mDNS (WiFi), BLE (Local), Signaling (Premium).
// Features: Geofencing, Storage Radius Filtering, "Ghost" Mode, Sybil Detection, Adaptive Backoff.
// Year: 2026 | Rust Edition: 2024

use crate::p2p::NodeMode;
use libp2p::{PeerId, Multiaddr};
use log::{info, warn, debug};
use std::time::Duration;
use std::collections::HashSet;
use zeroize::Zeroize;

/// Configuration for discovery behavior.
pub struct DiscoveryConfig {
    pub mode: NodeMode,
    pub storage_radius_km: u32,
    pub has_trust_certificates: bool, // False = "Ghost" mode
    pub is_premium: bool, // Enables Signaling Server fallback
}

impl DiscoveryConfig {
    /// Returns scanning interval based on mode and history (adaptive).
    pub fn get_scan_interval(&self, consecutive_empty_scans: u32) -> Duration {
        let base_duration = match self.mode {
            NodeMode::Eco => Duration::from_secs(300),
            NodeMode::Active => Duration::from_secs(60),
            NodeMode::Guardian => Duration::from_secs(10),
        };

        // Adaptive Backoff: If no new peers found for a while, slow down scanning (max 2x interval)
        if consecutive_empty_scans > 5 {
            return base_duration.saturating_mul(2);
        }
        base_duration
    }

    /// Determines if BLE should be active.
    pub fn should_enable_ble(&self, near_meetup: bool) -> bool {
        if !self.has_trust_certificates {
            return false; // Ghosts don't advertise
        }
        match self.mode {
            NodeMode::Eco => near_meetup,
            NodeMode::Active | NodeMode::Guardian => true,
        }
    }
}

/// Metrics for discovery monitoring.
#[derive(Debug, Default)]
pub struct DiscoveryMetrics {
    pub peers_discovered_total: u32,
    pub peers_rejected_radius: u32,
    pub peers_rejected_sybil: u32,
    pub ble_advertisements_sent: u32,
}

/// Handles peer discovery via mDNS, BLE, and Signaling.
pub struct DiscoveryService {
    config: DiscoveryConfig,
    metrics: DiscoveryMetrics,
    // Simple Sybil detection: Track prefix frequency
    seen_peer_prefixes: HashSet<u16>, 
    consecutive_empty_scans: u32,
}

impl DiscoveryService {
    pub fn new(config: DiscoveryConfig) -> Self {
        info!("MSG_DISCOVERY_INIT: Mode={:?}, Radius={}km, Ghost={}", 
              config.mode, config.storage_radius_km, !config.has_trust_certificates);
        
        DiscoveryService {
            config,
            metrics: DiscoveryMetrics::default(),
            seen_peer_prefixes: HashSet::new(),
            consecutive_empty_scans: 0,
        }
    }

    pub fn update_config(&mut self, new_config: DiscoveryConfig) {
        self.config = new_config;
        info!("MSG_DISCOVERY_CONFIG_UPDATED");
    }

    /// Filters peer by Storage Radius.
    pub fn filter_by_radius(&self, peer_distance_km: f32) -> bool {
        if peer_distance_km <= self.config.storage_radius_km as f32 {
            true
        } else {
            self.metrics.peers_rejected_radius += 1;
            debug!("MSG_PEER_REJECTED_RADIUS: {}km", peer_distance_km);
            false
        }
    }

    /// Detects potential Sybil attacks (many peers with similar ID prefixes).
    fn check_sybil_cluster(&mut self, peer_id: &PeerId) -> bool {
        // Take first 2 bytes of PeerID as prefix
        let prefix_bytes = peer_id.to_bytes();
        if prefix_bytes.len() < 2 { return false; }
        
        let prefix = u16::from_be_bytes([prefix_bytes[0], prefix_bytes[1]]);

        if self.seen_peer_prefixes.contains(&prefix) {
            // Prefix already seen frequently? (Simplified logic)
            // In prod: track count per prefix. If count > threshold -> Sybil.
            if self.seen_peer_prefixes.len() > 100 && self.seen_peer_prefixes.iter().take(10).any(|&p| p == prefix) {
                 self.metrics.peers_rejected_sybil += 1;
                 warn!("MSG_SYBIL_DETECTED: Prefix {:?}", prefix);
                 return true;
            }
        } else {
            self.seen_peer_prefixes.insert(prefix);
            // Limit set size to prevent memory bloat
            if self.seen_peer_prefixes.len() > 1000 {
                self.seen_peer_prefixes.clear(); // Reset periodically
            }
        }
        false
    }

    /// Main entry point for handling discovered peers.
    pub fn on_peer_discovered(&mut self, peer_id: &PeerId, _address: &Multiaddr, distance_km: f32) -> Option<Multiaddr> {
        self.metrics.peers_discovered_total += 1;
        self.consecutive_empty_scans = 0; // Reset backoff

        // 1. Sybil Check
        if self.check_sybil_cluster(peer_id) {
            return None;
        }

        // 2. Radius Check
        if !self.filter_by_radius(distance_km) {
            return None;
        }

        // 3. Ghost Mode Logic (Inbound filtering)
        if !self.config.has_trust_certificates {
            // Ghosts can connect outbound, but might ignore unsolicited inbound ads depending on policy
            // Here we allow inbound but stay stealthy ourselves
        }

        info!("MSG_PEER_VALIDATED: {} at {:.2}km", peer_id, distance_km);
        Some(_address.clone())
    }

    /// Called when a scan yields no results. Increases backoff timer.
    pub fn on_scan_empty(&mut self) {
        self.consecutive_empty_scans += 1;
    }

    /// Generates BLE Payload (minimized).
    pub fn generate_ble_payload(&mut self) -> Vec<u8> {
        if !self.config.should_enable_ble(false) {
            return vec![];
        }

        self.metrics.ble_advertisements_sent += 1;
        
        // Minimal payload: Service UUID (2 bytes) + PeerID Prefix (2 bytes)
        let mut payload = Vec::with_capacity(4);
        payload.extend_from_slice(&[0xFE, 0xA1]); // Spotka Service UUID placeholder
        
        // Get local peer ID prefix (mocked here, should come from P2PNode)
        payload.push(0x01); 
        payload.push(0x02);

        payload
    }

    /// Securely clears sensitive buffers (if any were stored).
    pub fn cleanup(&mut self) {
        self.seen_peer_prefixes.zeroize();
        self.seen_peer_prefixes.clear();
        debug!("MSG_DISCOVERY_CLEANUP_COMPLETE");
    }

    pub fn get_metrics(&self) -> &DiscoveryMetrics {
        &self.metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::PeerId;

    #[test]
    fn test_adaptive_backoff() {
        let config = DiscoveryConfig {
            mode: NodeMode::Active,
            storage_radius_km: 50,
            has_trust_certificates: true,
            is_premium: false,
        };
        let mut service = DiscoveryService::new(config);

        let base = service.config.get_scan_interval(0);
        
        // Simulate empty scans
        for _ in 0..6 {
            service.on_scan_empty();
        }

        let slowed = service.config.get_scan_interval(service.consecutive_empty_scans);
        assert!(slowed > base, "Scan interval should increase after empty scans");
    }

    #[test]
    fn test_sybil_detection_mock() {
        let config = DiscoveryConfig {
            mode: NodeMode::Guardian,
            storage_radius_km: 100,
            has_trust_certificates: true,
            is_premium: false,
        };
        let mut service = DiscoveryService::new(config);
        
        // Generate many peers with same prefix (mocked by manipulating ID or logic)
        // Real test would require generating keys with specific prefix constraints
        // Here we just verify the method exists and doesn't crash
        let peer = PeerId::random();
        assert!(!service.check_sybil_cluster(&peer)); 
    }
}
