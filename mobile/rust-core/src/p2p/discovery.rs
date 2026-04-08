// mobile/rust-core/src/p2p/discovery.rs
// Hybrid Node Discovery: mDNS (WiFi), BLE (Local), Signaling (Premium).
// Update: Strict Private Mesh Filtering (Whitelist Only), Invite Token Validation & Mapping.
// Architecture: No Public Gossip, Trust-Based Connections, Pending Peer Mapping.
// Year: 2026 | Rust Edition: 2024

use crate::p2p::NodeMode;
use libp2p::{PeerId, Multiaddr};
use log::{info, warn, debug};
use std::time::Duration;
use std::collections::{HashSet, HashMap};
use zeroize::Zeroize;

/// Configuration for discovery behavior.
pub struct DiscoveryConfig {
    pub mode: NodeMode,
    pub storage_radius_km: u32,
    pub has_trust_certificates: bool, // False = "Ghost" mode
    pub is_premium: bool,
}

impl DiscoveryConfig {
    pub fn get_scan_interval(&self, consecutive_empty_scans: u32) -> Duration {
        let base_duration = match self.mode {
            NodeMode::Eco => Duration::from_secs(300),
            NodeMode::Active => Duration::from_secs(60),
            NodeMode::Guardian => Duration::from_secs(10),
        };

        if consecutive_empty_scans > 5 {
            return base_duration.saturating_mul(2);
        }
        base_duration
    }

    pub fn should_enable_ble(&self, near_meetup: bool) -> bool {
        if !self.has_trust_certificates {
            return false; 
        }
        match self.mode {
            NodeMode::Eco => near_meetup,
            NodeMode::Active | NodeMode::Guardian => true,
        }
    }
}

#[derive(Debug, Default)]
pub struct DiscoveryMetrics {
    pub peers_discovered_total: u32,
    pub peers_rejected_not_trusted: u32,
    pub peers_rejected_invalid_invite: u32,
    pub peers_rejected_radius: u32,
    pub peers_rejected_sybil: u32,
    pub ble_advertisements_sent: u32,
}

/// Result of a discovery attempt.
/// Tells the upper layer how to handle the new connection.
#[derive(Debug, Clone)]
pub enum DiscoveryResult {
    /// Peer is a verified friend (Ping). Full access.
    Trusted(PeerId, String), // PeerId, UserHash
    /// Peer is a guest with valid invite. Limited access.
    InvitedGuest(PeerId, String), // PeerId, UserHash (from token context or handshake)
    /// Peer rejected.
    Rejected(String), // Reason
}

/// Handles peer discovery with strict Trust Graph enforcement.
pub struct DiscoveryService {
    config: DiscoveryConfig,
    metrics: DiscoveryMetrics,
    
    // Whitelist of trusted PeerIDs (verified via Ping)
    // Map: PeerID -> UserHash
    trusted_peers: HashSet<PeerId>, 
    
    // Active Invites: Temporary tokens allowing non-trusted peers to connect
    // Map: InviteToken (hash) -> (UserHash, ExpiryTimestamp)
    // Storing UserHash allows us to know WHO is connecting even before full handshake
    active_invites: HashMap<String, (String, u64)>,
    
    // NEW: Pending Peers mapping.
    // When a peer connects via token, we temporarily store their PeerID -> UserHash here
    // until the upper layer (P2PNode) confirms the connection and updates the main TrustGraph.
    pending_peer_map: HashMap<PeerId, String>,

    seen_peer_prefixes: HashSet<u16>, 
    consecutive_empty_scans: u32,
}

impl DiscoveryService {
    pub fn new(config: DiscoveryConfig) -> Self {
        info!("MSG_DISCOVERY_INIT: Mode={:?}, Radius={}km, PrivateMesh={}", 
              config.mode, config.storage_radius_km, !config.has_trust_certificates);
        
        DiscoveryService {
            config,
            metrics: DiscoveryMetrics::default(),
            trusted_peers: HashSet::new(),
            active_invites: HashMap::new(),
            pending_peer_map: HashMap::new(),
            seen_peer_prefixes: HashSet::new(),
            consecutive_empty_scans: 0,
        }
    }

    pub fn update_config(&mut self, new_config: DiscoveryConfig) {
        self.config = new_config;
        info!("MSG_DISCOVERY_CONFIG_UPDATED");
    }

    /// Adds a peer to the permanent trust list (called after successful Ping).
    pub fn add_trusted_peer(&mut self, peer_id: PeerId, user_hash: String) {
        self.trusted_peers.insert(peer_id);
        // Ensure they are not in pending anymore
        self.pending_peer_map.remove(&peer_id);
        info!("MSG_PEER_TRUSTED_ADDED: {} ({})", peer_id, user_hash);
    }

    /// Registers a temporary invite token.
    /// Associates the token with a specific UserHash (the intended recipient).
    pub fn register_invite_token(&mut self, token_hash: String, user_hash: String, expiry_timestamp: u64) {
        self.active_invites.insert(token_hash, (user_hash, expiry_timestamp));
        debug!("MSG_INVITE_TOKEN_REGISTERED: For {} Expires at {}", user_hash, expiry_timestamp);
    }

    pub fn filter_by_radius(&self, peer_distance_km: f32) -> bool {
        if peer_distance_km <= self.config.storage_radius_km as f32 {
            true
        } else {
            self.metrics.peers_rejected_radius += 1;
            debug!("MSG_PEER_REJECTED_RADIUS: {}km", peer_distance_km);
            false
        }
    }

    fn check_sybil_cluster(&mut self, peer_id: &PeerId) -> bool {
        let prefix_bytes = peer_id.to_bytes();
        if prefix_bytes.len() < 2 { return false; }
        
        let prefix = u16::from_be_bytes([prefix_bytes[0], prefix_bytes[1]]);

        if self.seen_peer_prefixes.contains(&prefix) {
            if self.seen_peer_prefixes.len() > 100 && self.seen_peer_prefixes.iter().take(10).any(|&p| p == prefix) {
                 self.metrics.peers_rejected_sybil += 1;
                 warn!("MSG_SYBIL_DETECTED: Prefix {:?}", prefix);
                 return true;
            }
        } else {
            self.seen_peer_prefixes.insert(prefix);
            if self.seen_peer_prefixes.len() > 1000 {
                self.seen_peer_prefixes.clear(); 
            }
        }
        false
    }

    /// MAIN ENTRY POINT: Strict filtering for Private Mesh.
    /// Returns DiscoveryResult indicating how to handle the peer.
    pub fn on_peer_discovered(
        &mut self, 
        peer_id: &PeerId, 
        address: &Multiaddr, 
        distance_km: f32,
        invite_token: Option<String>,
    ) -> DiscoveryResult {
        self.metrics.peers_discovered_total += 1;
        self.consecutive_empty_scans = 0;

        // 1. Sybil Check
        if self.check_sybil_cluster(peer_id) {
            return DiscoveryResult::Rejected("Sybil detected".to_string());
        }

        // 2. Radius Check
        if !self.filter_by_radius(distance_km) {
            return DiscoveryResult::Rejected("Out of radius".to_string());
        }

        // 3. TRUST CHECK
        
        // Case A: Peer is already trusted (Friend via Ping)
        if self.trusted_peers.contains(peer_id) {
            info!("MSG_PEER_TRUSTED_CONNECTED: {} at {:.2}km", peer_id, distance_km);
            // Retrieve user hash if possible (in real impl, lookup from DB/Graph)
            // For now, we return success and let upper layer handle details
            return DiscoveryResult::Trusted(*peer_id, "known_user".to_string());
        }

        // Case B: Peer presents an Invite Token
        if let Some(token) = invite_token {
            if let Some((user_hash, expiry)) = self.active_invites.get(&token) {
                let now = chrono::Utc::now().timestamp() as u64;
                if now < *expiry {
                    info!("MSG_PEER_INVITE_ACCEPTED: {} (Token Valid for {})", peer_id, user_hash);
                    
                    // Register in pending map so upper layer knows who connected
                    self.pending_peer_map.insert(*peer_id, user_hash.clone());
                    
                    return DiscoveryResult::InvitedGuest(*peer_id, user_hash.clone());
                } else {
                    self.metrics.peers_rejected_invalid_invite += 1;
                    warn!("MSG_PEER_INVITE_EXPIRED: {}", peer_id);
                    return DiscoveryResult::Rejected("Token expired".to_string());
                }
            } else {
                self.metrics.peers_rejected_invalid_invite += 1;
                warn!("MSG_PEER_INVITE_INVALID: {}", peer_id);
                return DiscoveryResult::Rejected("Invalid token".to_string());
            }
        }

        // Case C: Unknown peer, no token -> REJECT
        self.metrics.peers_rejected_not_trusted += 1;
        debug!("MSG_PEER_REJECTED_NOT_TRUSTED: {} (No Token, Not Friend)", peer_id);
        return DiscoveryResult::Rejected("Not trusted".to_string());
    }

    pub fn on_scan_empty(&mut self) {
        self.consecutive_empty_scans += 1;
    }

    pub fn generate_ble_payload(&mut self) -> Vec<u8> {
        if !self.config.should_enable_ble(false) {
            return vec![];
        }

        self.metrics.ble_advertisements_sent += 1;
        
        let mut payload = Vec::with_capacity(4);
        payload.extend_from_slice(&[0xFE, 0xA1]); 
        payload.push(0x01); 
        payload.push(0x02);

        payload
    }

    pub fn cleanup(&mut self) {
        self.trusted_peers.zeroize();
        self.trusted_peers.clear();
        self.active_invites.zeroize();
        self.active_invites.clear();
        self.pending_peer_map.zeroize();
        self.pending_peer_map.clear();
        self.seen_peer_prefixes.zeroize();
        self.seen_peer_prefixes.clear();
        debug!("MSG_DISCOVERY_CLEANUP_COMPLETE");
    }

    pub fn get_metrics(&self) -> &DiscoveryMetrics {
        &self.metrics
    }
    
    // Helper to retrieve pending user hash by PeerID
    pub fn get_pending_user_hash(&self, peer_id: &PeerId) -> Option<String> {
        self.pending_peer_map.get(peer_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::PeerId;

    #[test]
    fn test_private_mesh_rejection() {
        let config = DiscoveryConfig {
            mode: NodeMode::Active,
            storage_radius_km: 50,
            has_trust_certificates: true,
            is_premium: false,
        };
        let mut service = DiscoveryService::new(config);
        let peer = PeerId::random();
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/4001".parse().unwrap();

        let res = service.on_peer_discovered(&peer, &addr, 1.0, None);
        assert!(matches!(res, DiscoveryResult::Rejected(_)));
        assert_eq!(service.metrics.peers_rejected_not_trusted, 1);
    }

    #[test]
    fn test_trusted_peer_acceptance() {
        let config = DiscoveryConfig {
            mode: NodeMode::Active,
            storage_radius_km: 50,
            has_trust_certificates: true,
            is_premium: false,
        };
        let mut service = DiscoveryService::new(config);
        let peer = PeerId::random();
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/4001".parse().unwrap();

        service.add_trusted_peer(peer, "user_hash_123".to_string());

        let res = service.on_peer_discovered(&peer, &addr, 1.0, None);
        assert!(matches!(res, DiscoveryResult::Trusted(_, _)));
    }

    #[test]
    fn test_invite_token_acceptance_and_mapping() {
        let config = DiscoveryConfig {
            mode: NodeMode::Active,
            storage_radius_km: 50,
            has_trust_certificates: true,
            is_premium: false,
        };
        let mut service = DiscoveryService::new(config);
        let peer = PeerId::random();
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/4001".parse().unwrap();
        let token = "valid_invite_token".to_string();
        let target_user = "target_user_hash".to_string();
        
        let expiry = (chrono::Utc::now().timestamp() + 3600) as u64;
        service.register_invite_token(token.clone(), target_user.clone(), expiry);

        let res = service.on_peer_discovered(&peer, &addr, 1.0, Some(token));
        
        assert!(matches!(res, DiscoveryResult::InvitedGuest(_, _)));
        // Verify pending map was updated
        assert_eq!(service.get_pending_user_hash(&peer), Some(target_user));
    }
}
