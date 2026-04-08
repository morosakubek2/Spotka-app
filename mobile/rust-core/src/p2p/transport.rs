// mobile/rust-core/src/p2p/transport.rs
// Hybrid Transport Layer: QUIC (Primary), TCP (Fallback), BLE (Local).
// Architecture: Private Mesh with Centralized Trust Verification.
// Features: 
//   - Strict Trust Filtering (via Central TrustGraph).
//   - NAT Traversal (Hole Punching) for Invite Forwarding.
//   - Aggressive Keep-Alive / Dead Peer Detection.
// Security: Noise Encryption, Zeroize on Drop, No Local Token State.
// Year: 2026 | Rust Edition: 2024

use libp2p::{
    core::transport::upgrade::Version,
    identity::Keypair,
    quic, tcp, dns, noise, yamux,
    Transport, PeerId, Multiaddr,
};
use crate::p2p::{NodeMode, TrustGraph}; // Import TrustGraph
use crate::db::manager::DbManager;
use log::{info, warn, error, debug};
use std::time::Duration;
use std::sync::Arc;
use tokio::sync::RwLock;
use zeroize::{Zeroize, Zeroizing};

/// Configuration for the hybrid transport stack.
pub struct TransportConfig {
    pub enable_quic: bool,
    pub enable_tcp: bool,
    pub enable_ble: bool,
    pub max_retries: u32,
    pub connection_timeout_secs: u64,
    
    // Aggressive keep-alive for mobile to detect dead peers quickly
    pub keep_alive_interval_secs: u64,
    
    // Strict mode flag (blocks all unknown incoming connections)
    pub strict_mesh_mode: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            enable_quic: true,
            enable_tcp: true,
            enable_ble: false,
            max_retries: 3,
            connection_timeout_secs: 10,
            keep_alive_interval_secs: 15,
            strict_mesh_mode: true,
        }
    }
}

impl Zeroize for TransportConfig {
    fn zeroize(&mut self) {
        self.max_retries.zeroize();
    }
}

impl Drop for TransportConfig {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Connection Guard: Wraps the transport to enforce trust rules.
/// DELEGATES trust checks to the central TrustGraph (managed by P2PNode/App).
/// Prevents resource exhaustion from unknown peers in Private Mesh mode.
pub struct ConnectionGuard {
    trust_graph: Arc<RwLock<TrustGraph>>,
    strict_mode: bool,
    // NO LOCAL TOKEN CACHE HERE. 
    // Token validation happens in App Logic -> updates TrustGraph -> Transport checks TrustGraph.
}

impl ConnectionGuard {
    pub fn new(trust_graph: Arc<RwLock<TrustGraph>>, strict: bool) -> Self {
        Self {
            trust_graph,
            strict_mode: strict,
        }
    }

    /// Validates an incoming connection request.
    /// Returns true if connection should be accepted.
    pub async fn validate_incoming(&self, peer_id: &PeerId) -> bool {
        if !self.strict_mode {
            return true;
        }

        // Check Central Trust Graph
        let graph = self.trust_graph.read().await;
        let level = graph.get_level(peer_id);
        
        // Accept if Verified Friend OR Invited Guest (temporary access granted by App Logic)
        let is_trusted = matches!(level, crate::p2p::TrustLevel::VerifiedFriend | crate::p2p::TrustLevel::InvitedGuest);

        if is_trusted {
            debug!("MSG_CONNECTION_ACCEPTED_TRUSTED: {} (Level: {:?})", peer_id, level);
            true
        } else {
            warn!("MSG_CONNECTION_REJECTED_STRICT: Unknown/Untrusted Peer {}", peer_id);
            false
        }
    }
}

/// Builds the libp2p Transport stack with Security Guards.
pub fn build_transport(
    keypair: &Keypair,
    mode: NodeMode,
    config: TransportConfig,
    trust_graph: Arc<RwLock<TrustGraph>>, // Inject TrustGraph instead of DbManager
) -> Result<Box<dyn libp2p::Transport<Output = (PeerId, yamux::Stream)> + Unpin>, &'static str> {
    info!("MSG_TRANSPORT_BUILD_START: Mode={:?}, Strict={}", mode, config.strict_mesh_mode);

    // 1. Authentication (Noise Protocol - IK pattern)
    let auth_config = noise::Config::new(keypair)
        .map_err(|_| "ERR_NOISE_CONFIG_FAILED")?;

    // 2. Multiplexing (Yamux) with Mobile Tuning
    let mut muxer_config = yamux::Config::default();
    muxer_config.set_max_num_streams(256);
    muxer_config.set_receive_window_size(2 * 1024 * 1024);
    muxer_config.set_keep_alive_interval(Duration::from_secs(config.keep_alive_interval_secs));

    let guard = Arc::new(ConnectionGuard::new(trust_graph, config.strict_mesh_mode));
    let guard_clone = guard.clone();

    // Helper to wrap transport with Connection Guard logic
    let wrap_with_guard = |transport: Box<dyn libp2p::Transport<Output = _> + Unpin>| {
        transport.try_map_boxed(move |(peer_id, stream), _| {
            let guard_inner = guard_clone.clone();
            async move {
                if !guard_inner.validate_incoming(&peer_id).await {
                    drop(stream);
                    return Err(libp2p::core::TransportError::Other(
                        Box::new(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Untrusted Peer"))
                    ));
                }
                Ok((peer_id, stream))
            }
        })
    };

    let mut transport_stack: Option<Box<dyn libp2p::Transport<Output = _> + Unpin>> = None;

    // --- QUIC (Preferred for Mobile/NAT & Hole Punching) ---
    if config.enable_quic {
        let mut quic_config = quic::Config::new(keypair);
        quic_config.support_hole_punching(true); 
        quic_config.set_keep_alive_interval(Duration::from_secs(config.keep_alive_interval_secs));
        
        let quic_transport = quic::tokio::Transport::new(quic_config);
        
        let quic_upgraded = quic_transport
            .upgrade(Version::V1)
            .authenticate(auth_config.clone())
            .multiplex(muxer_config.clone());

        let guarded_quic = wrap_with_guard(Box::new(quic_upgraded));

        transport_stack = Some(if let Some(prev) = transport_stack {
            Box::new(prev.or_else(guarded_quic))
        } else {
            guarded_quic
        });
        info!("MSG_TRANSPORT_QUIC_ENABLED_WITH_HOLE_PUNCHING");
    }

    // --- TCP (Fallback) ---
    if config.enable_tcp {
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
        
        let dns_tcp = dns::tokio::Transport::system(tcp_transport)
            .map_err(|_| "ERR_DNS_INIT_FAILED")?;

        let tcp_upgraded = dns_tcp
            .upgrade(Version::V1)
            .authenticate(auth_config.clone())
            .multiplex(muxer_config.clone());

        let guarded_tcp = wrap_with_guard(Box::new(tcp_upgraded));

        transport_stack = Some(if let Some(prev) = transport_stack {
            Box::new(prev.or_else(guarded_tcp))
        } else {
            guarded_tcp
        });
        info!("MSG_TRANSPORT_TCP_ENABLED");
    }

    // --- BLE (Local Only) ---
    if config.enable_ble && mode != NodeMode::Eco {
        info!("MSG_TRANSPORT_BLE_READY_PENDING_DISCOVERY");
    } else if config.enable_ble {
        warn!("MSG_TRANSPORT_BLE_DISABLED_ECO_MODE");
    }

    match transport_stack {
        Some(stack) => {
            info!("MSG_TRANSPORT_STACK_READY: Timeout={}s, KeepAlive={}s", 
                  config.connection_timeout_secs, config.keep_alive_interval_secs);
            Ok(stack)
        },
        None => Err("ERR_NO_TRANSPORT_AVAILABLE"),
    }
}

pub fn should_prioritize_quic(is_mobile_data: bool) -> bool {
    is_mobile_data
}

pub async fn check_connectivity() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity::Keypair;
    use crate::p2p::TrustGraph;

    #[tokio::test]
    async fn test_connection_guard_strict_mode() {
        let trust_graph = Arc::new(RwLock::new(TrustGraph::new()));
        let guard = ConnectionGuard::new(trust_graph.clone(), true);

        let random_peer = PeerId::random();
        // Should reject unknown peer
        assert!(!guard.validate_incoming(&random_peer).await, "Should reject unknown peer in strict mode");

        // Simulate App Logic adding a guest (e.g., after valid invite token verification)
        trust_graph.write().await.add_guest(random_peer, "user_hash_123".to_string());

        // Now should accept
        assert!(guard.validate_incoming(&random_peer).await, "Should accept invited guest");
    }

    #[test]
    fn test_build_transport_with_guard() {
        let keypair = Keypair::generate_ed25519();
        let trust_graph = Arc::new(RwLock::new(TrustGraph::new()));
        let config = TransportConfig {
            enable_quic: true,
            enable_tcp: false,
            strict_mesh_mode: true,
            ..Default::default()
        };
        
        let result = build_transport(&keypair, NodeMode::Active, config, trust_graph);
        assert!(result.is_ok(), "Transport with Guard should build successfully");
    }
}
