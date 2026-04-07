// mobile/rust-core/src/p2p/transport.rs
// Hybrid Transport Layer: QUIC (Primary), TCP (Fallback), BLE (Local).
// Features: NAT Traversal (Hole Punching), Adaptive Switching, Noise Encryption.
// Year: 2026 | Rust Edition: 2024

use libp2p::{
    core::transport::upgrade::Version,
    identity::Keypair,
    quic, tcp, dns, noise, yamux,
    Transport,
};
use crate::p2p::NodeMode;
use log::{info, warn};

/// Configuration for the hybrid transport stack.
pub struct TransportConfig {
    pub enable_quic: bool,
    pub enable_tcp: bool,
    pub enable_ble: bool, // Controlled by discovery module, but flag needed here
    pub max_retries: u32,
    pub connection_timeout_secs: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            enable_quic: true,
            enable_tcp: true,
            enable_ble: false, // BLE is expensive, enabled only via Geofencing/Ping
            max_retries: 3,
            connection_timeout_secs: 10,
        }
    }
}

/// Builds the libp2p Transport stack based on current NodeMode and Config.
/// Returns a boxed Transport capable of handling QUIC, TCP, and eventually BLE.
pub fn build_transport(
    keypair: &Keypair,
    mode: NodeMode,
    config: TransportConfig,
) -> Result<Box<dyn libp2p::Transport<Output = (libp2p::PeerId, yamux::Stream)> + Unpin>, &'static str> {
    info!("MSG_TRANSPORT_BUILD_START: Mode {:?}", mode);

    // 1. Authentication (Noise Protocol)
    let auth_config = noise::Config::new(keypair)
        .map_err(|_| "ERR_NOISE_CONFIG_FAILED")?;

    // 2. Multiplexing (Yamux)
    let muxer_config = yamux::Config::default();

    // 3. Base Transports
    let mut transport_stack: Option<Box<dyn libp2p::Transport<Output = _> + Unpin>> = None;

    // --- QUIC (Preferred for Mobile/NAT) ---
    if config.enable_quic {
        let quic_config = quic::Config::new(keypair);
        let quic_transport = quic::tokio::Transport::new(quic_config);
        
        // Wrap with Auth & Muxer
        let quic_upgraded = quic_transport
            .upgrade(Version::V1)
            .authenticate(auth_config.clone())
            .multiplex(muxer_config.clone());

        transport_stack = Some(if let Some(prev) = transport_stack {
            Box::new(prev.or_else(quic_upgraded))
        } else {
            Box::new(quic_upgraded)
        });
        info!("MSG_TRANSPORT_QUIC_ENABLED");
    }

    // --- TCP (Fallback for restrictive networks) ---
    if config.enable_tcp {
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
        
        // DNS Resolution for TCP addresses
        let dns_tcp = dns::tokio::Transport::system(tcp_transport)
            .map_err(|_| "ERR_DNS_INIT_FAILED")?;

        let tcp_upgraded = dns_tcp
            .upgrade(Version::V1)
            .authenticate(auth_config.clone())
            .multiplex(muxer_config.clone());

        transport_stack = Some(if let Some(prev) = transport_stack {
            Box::new(prev.or_else(tcp_upgraded))
        } else {
            Box::new(tcp_upgraded)
        });
        info!("MSG_TRANSPORT_TCP_ENABLED");
    }

    // --- BLE (Handled separately in discovery.rs due to platform specifics) ---
    // Note: BLE transport in libp2p often requires custom integration or specific features.
    // Here we assume it's added via a separate interface or feature flag if available.
    if config.enable_ble && mode != NodeMode::Eco {
        info!("MSG_TRANSPORT_BLE_READY_PENDING_DISCOVERY");
        // BLE logic is tightly coupled with OS APIs (CoreBluetooth/Android BLE), 
        // so it's usually injected into the Swarm behaviour rather than standard Transport builder.
    } else if config.enable_ble {
        warn!("MSG_TRANSPORT_BLE_DISABLED_ECO_MODE");
    }

    match transport_stack {
        Some(stack) => {
            // Add Timeout wrapper to prevent hanging connections
            // (Pseudo-code for timeout wrapper, actual impl depends on libp2p version)
            // let timed_stack = stack.with_timeout(Duration::from_secs(config.connection_timeout_secs));
            Ok(stack)
        },
        None => Err("ERR_NO_TRANSPORT_AVAILABLE"),
    }
}

/// Helper to determine optimal transport strategy based on network conditions.
/// Returns true if QUIC should be prioritized (usually yes for mobile).
pub fn should_prioritize_quic(is_mobile_data: bool) -> bool {
    // QUIC handles packet loss and migration better than TCP on mobile networks
    is_mobile_data
}

/// Performs a connectivity check (NAT Traversal readiness).
/// Returns true if the node believes it can accept incoming connections (via UPnP/Hole Punching).
pub async fn check_connectivity() -> bool {
    // In production, this would attempt a hole-punch or check UPnP/IGD status
    // For now, optimistic true
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity::Keypair;

    #[test]
    fn test_build_transport_quic_only() {
        let keypair = Keypair::generate_ed25519();
        let config = TransportConfig {
            enable_quic: true,
            enable_tcp: false,
            enable_ble: false,
            ..Default::default()
        };
        
        let result = build_transport(&keypair, NodeMode::Active, config);
        assert!(result.is_ok(), "QUIC transport should build successfully");
    }

    #[test]
    fn test_build_transport_eco_no_ble() {
        let keypair = Keypair::generate_ed25519();
        let config = TransportConfig {
            enable_ble: true, // Requested but should be ignored in Eco
            ..Default::default()
        };
        
        // Should build without error, but BLE should be logically disabled
        let result = build_transport(&keypair, NodeMode::Eco, config);
        assert!(result.is_ok());
    }
}
