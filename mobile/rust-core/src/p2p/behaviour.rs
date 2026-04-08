// mobile/rust-core/src/p2p/behaviour.rs
// P2P Network Behaviour: The "Brain" of the Private Mesh Node.
// Features: 
//   - Integrates Transport, Discovery, and Sync layers.
//   - Enforces "Friends Only" meeting rules.
//   - Handles Invite Forwarding logic based on Trust Graph.
//   - Manages Connection Lifecycle (Accept/Reject).
// Year: 2026 | Rust Edition: 2024

use crate::p2p::{
    discovery::{DiscoveryService, DiscoveryConfig},
    sync::SyncManager,
    transport::ConnectionGuard,
    protocol::{MessageEnvelope, MessageType, InvitePayload, ParticipationUpdatePayload, ParticipantStatus, InviteRejectPayload, RejectReason},
    NodeMode, TrustGraph, TrustLevel, PacketAction,
};
use crate::db::manager::DbManager;
use crate::crypto::identity::Identity;
use libp2p::{PeerId, Multiaddr};
use log::{info, warn, debug, error};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;

/// Events emitted by the Behaviour to the upper application layer (UI/AppController).
#[derive(Debug, Clone)]
pub enum P2PEvent {
    /// A new peer connected (trusted or guest).
    PeerConnected(PeerId),
    /// A new meeting invite received.
    InviteReceived { meeting_id: String, organizer: String },
    /// Participation status update received (public only).
    ParticipationUpdated { meeting_id: String, user: String, status: String },
    /// Meeting is full, invite invalid, or access restricted.
    Error { code: String, message: String },
    /// Explicit invite rejection received from network.
    InviteRejected { meeting_id: String, reason: String },
}

/// Main Behaviour struct coordinating the Private Mesh logic.
/// In a real libp2p setup, this would derive `NetworkBehaviour` and compose sub-behaviours.
pub struct SpotkaBehaviour {
    identity: Identity,
    mode: NodeMode,
    db_manager: Arc<RwLock<DbManager>>,
    
    // Sub-modules
    discovery: DiscoveryService,
    sync_manager: Arc<RwLock<SyncManager>>,
    trust_graph: Arc<RwLock<TrustGraph>>,
    connection_guard: Arc<ConnectionGuard>,

    // State buffers
    pending_events: VecDeque<P2PEvent>,
    outgoing_messages: VecDeque<(PeerId, MessageEnvelope)>,
    
    // Mapping for active connections: PeerID -> Address
    active_connections: HashMap<PeerId, Multiaddr>,
}

impl SpotkaBehaviour {
    pub fn new(
        identity: Identity,
        db_manager: Arc<RwLock<DbManager>>,
        mode: NodeMode,
        storage_radius_km: u32,
    ) -> Self {
        let config = DiscoveryConfig {
            mode,
            storage_radius_km,
            has_trust_certificates: true, // Assume verified for now
            is_premium: false,
        };

        let discovery = DiscoveryService::new(config);
        
        // Mock Chain for SyncManager initialization
        // In prod: Pass real AppChain instance
        let chain_mock = Arc::new(RwLock::new(
            crate::chain::AppChain::new(db_manager.clone())
                .await.unwrap_or_else(|_| panic!("Failed to init AppChain"))
        ));

        let sync_manager = Arc::new(RwLock::new(SyncManager::new(
            db_manager.clone(), 
            chain_mock, 
            identity.clone(), 
            storage_radius_km
        )));
        
        let trust_graph = Arc::new(RwLock::new(TrustGraph::new()));
        // ConnectionGuard now uses TrustGraph directly (via injection in transport.rs build)
        // But here we might need a reference if we manage tokens here. 
        // For consistency with transport.rs update, we assume TrustGraph is the source of truth.
        let connection_guard = Arc::new(ConnectionGuard::new(trust_graph.clone(), true));

        SpotkaBehaviour {
            identity,
            mode,
            db_manager,
            discovery,
            sync_manager,
            trust_graph,
            connection_guard,
            pending_events: VecDeque::new(),
            outgoing_messages: VecDeque::new(),
            active_connections: HashMap::new(),
        }
    }

    // --- CONNECTION LIFECYCLE ---

    /// Called when a new connection is established.
    /// Validates the peer against Trust Graph.
    pub fn on_connection_established(&mut self, peer_id: &PeerId, address: &Multiaddr) {
        info!("MSG_CONNECTION_ESTABLISHED: {} at {}", peer_id, address);

        // Trust level check is already done by ConnectionGuard in transport layer.
        // If we are here, the connection is allowed (Verified or Guest via Token).
        
        let level = futures::executor::block_on(async {
            self.trust_graph.read().await.get_level(peer_id)
        });

        if level == TrustLevel::None {
            // This implies a temporary guest access via token that hasn't been added to graph yet
            // Or a race condition. We allow it but log.
            debug!("MSG_CONNECTION_GUEST_ALLOWED_VIA_TOKEN: {}", peer_id);
        }

        self.active_connections.insert(*peer_id, address.clone());
        self.pending_events.push_back(P2PEvent::PeerConnected(*peer_id));
    }

    /// Called when a connection closes.
    pub fn on_connection_closed(&mut self, peer_id: &PeerId) {
        self.active_connections.remove(peer_id);
        debug!("MSG_CONNECTION_CLOSED: {}", peer_id);
        
        // Optional: Cleanup guest status if ephemeral
    }

    // --- MESSAGE HANDLING ---

    /// Processes an incoming message envelope.
    pub async fn on_message_received(&mut self, peer_id: &PeerId, packet: MessageEnvelope) {
        // 1. Pre-Filter: Check Trust & Type
        match self.filter_packet(peer_id, &packet).await {
            Ok(PacketAction::Process) => {},
            Ok(PacketAction::Forward(target_peer)) => {
                self.forward_message(target_peer, packet).await;
                return;
            },
            Err(e) => {
                warn!("MSG_PACKET_DROPPED: {} from {}", e, peer_id);
                return;
            }
            _ => return,
        }

        // 2. Dispatch based on Message Type
        match packet.header.msg_type {
            MessageType::Invite => {
                self.handle_invite_message(peer_id, packet).await;
            },
            MessageType::InviteReject => {
                self.handle_invite_reject_message(peer_id, packet).await;
            },
            MessageType::ParticipationUpdate => {
                self.handle_participation_message(peer_id, packet).await;
            },
            MessageType::SyncRequest | MessageType::SyncResponse => {
                // Delegate to SyncManager directly (logic omitted for brevity)
                // self.sync_manager.write().await.handle_sync_packet(...).await;
            },
            MessageType::Gossip => {
                // Should be blocked by filter
                warn!("MSG_GOSSIP_BLOCKED_BEHAVIOUR: From {}", peer_id);
            },
            _ => {
                debug!("MSG_UNKNOWN_MESSAGE_TYPE: {:?}", packet.header.msg_type);
            }
        }
    }

    async fn filter_packet(&self, peer_id: &PeerId, packet: &MessageEnvelope) -> Result<PacketAction, &'static str> {
        // Hard block Gossip in Free/Private mode
        if matches!(packet.header.msg_type, MessageType::Gossip) {
            return Err("ERR_GOSSIP_DISABLED");
        }

        let trust_level = self.trust_graph.read().await.get_level(peer_id);

        match &packet.header.msg_type {
            MessageType::Invite => {
                // Invites allowed from Verified Friends (forwarding) or Guests (if they have token context)
                if trust_level == TrustLevel::VerifiedFriend {
                    return Ok(PacketAction::Process);
                }
                // If Guest, allow processing (signature/token validation happens inside handler)
                Ok(PacketAction::Process)
            },
            MessageType::InviteReject => {
                // Accept rejects from anyone we sent an invite to
                Ok(PacketAction::Process)
            },
            MessageType::ParticipationUpdate => {
                // Only Verified Friends or Organizer can send updates
                if trust_level == TrustLevel::VerifiedFriend {
                    return Ok(PacketAction::Process);
                }
                Err("ERR_UNTRUSTED_PARTICIPATION_UPDATE")
            },
            _ => {
                // Default policy: Only Verified Friends
                if trust_level == TrustLevel::VerifiedFriend {
                    Ok(PacketAction::Process)
                } else {
                    Err("ERR_UNTRUSTED_SENDER")
                }
            }
        }
    }

    async fn handle_invite_message(&mut self, peer_id: &PeerId, packet: MessageEnvelope) {
        let payload: InvitePayload = match packet.get_payload() {
            Ok(p) => p,
            Err(_) => {
                warn!("MSG_INVITE_DESERIALIZE_FAILED");
                return;
            }
        };

        // Check "Friends Only" Rule
        if payload.is_friends_only {
            // If this flag is set, ONLY the Organizer can send invites directly.
            // Forwarding by intermediaries is banned unless they verify direct friendship (complex).
            // Simplified rule: If I am not the organizer, and I received this, and it's friends_only,
            // then if I am an intermediate node, I should NOT forward it further.
            // If I am the recipient, I accept.
            
            let sender_is_organizer = peer_id.to_string() == payload.organizer_id_hash;
            
            if !sender_is_organizer && payload.recipient_id_hash != self.identity.phone_hash {
                // I am an intermediate node, and sender is not organizer.
                // This violates the strict "Friends Only" direct-send rule.
                warn!("MSG_INVITE_BLOCKED_FRIENDS_ONLY: Forwarding not allowed by non-organizer");
                
                // Optionally send Reject back to sender
                self.send_invite_reject(peer_id, &payload.meeting_id_hash, RejectReason::FriendsOnlyRestricted).await;
                return; 
            }
        }

        // Is this invite for me?
        if payload.recipient_id_hash == self.identity.phone_hash {
            // Process locally
            match self.sync_manager.read().await.handle_invite(&payload).await {
                Ok(_) => {
                    self.pending_events.push_back(P2PEvent::InviteReceived {
                        meeting_id: payload.meeting_id_hash,
                        organizer: payload.organizer_id_hash,
                    });
                },
                Err(e) => {
                    // If error is "Meeting Full", send reject back
                    if e == "ERR_MEETING_FULL" {
                        self.send_invite_reject(peer_id, &payload.meeting_id_hash, RejectReason::MeetingFull).await;
                    }
                    self.pending_events.push_back(P2PEvent::Error { 
                        code: "INVITE_ERROR".to_string(), 
                        message: e.to_string() 
                    });
                }
            }
        } else {
            // Forwarding Logic
            // Check if I know the recipient in my Trust Graph
            let graph = self.trust_graph.read().await;
            if let Some(target_peer) = graph.get_peer_for_user(&payload.recipient_id_hash) {
                drop(graph);
                // Queue for forwarding
                self.outgoing_messages.push_back((target_peer, packet));
                info!("MSG_INVITE_FORWARDED: To {} via {}", payload.recipient_id_hash, peer_id);
            } else {
                drop(graph);
                warn!("MSG_INVITE_DROP_UNKNOWN_RECIPIENT: {}", payload.recipient_id_hash);
                // Optionally notify sender that recipient is unknown (Silent drop preferred for privacy)
            }
        }
    }

    async fn handle_invite_reject_message(&mut self, _peer_id: &PeerId, packet: MessageEnvelope) {
        let payload: InviteRejectPayload = match packet.get_payload() {
            Ok(p) => p,
            Err(_) => return,
        };

        // Notify SyncManager to cleanup state
        if let Err(e) = self.sync_manager.read().await.handle_invite_reject(&payload).await {
            warn!("MSG_ERROR_HANDLING_REJECT: {}", e);
        }

        // Notify UI
        let reason_str = format!("{:?}", payload.reason_code); // In prod: map enum to string
        self.pending_events.push_back(P2PEvent::InviteRejected {
            meeting_id: payload.meeting_id_hash,
            reason: reason_str,
        });
    }

    async fn handle_participation_message(&mut self, peer_id: &PeerId, packet: MessageEnvelope) {
        let payload: ParticipationUpdatePayload = match packet.get_payload() {
            Ok(p) => p,
            Err(_) => return,
        };

        // Privacy Check: Ignore Interested status
        if payload.status == ParticipantStatus::Interested {
            return; 
        }

        // Validate & Store
        match self.sync_manager.read().await.handle_participation_update(&payload, &[0u8; 32]).await {
            Ok(should_propagate) => {
                self.pending_events.push_back(P2PEvent::ParticipationUpdated {
                    meeting_id: payload.meeting_id_hash,
                    user: payload.user_id_hash,
                    status: format!("{:?}", payload.status),
                });

                // If propagation needed (e.g., Guardian mode), queue for broadcast
                if should_propagate && self.mode == NodeMode::Guardian {
                    // Broadcast logic to other trusted peers (excluding sender)
                    // Implementation omitted for brevity
                }
            },
            Err(e) => {
                warn!("MSG_PARTICIPATION_UPDATE_ERROR: {}", e);
            }
        }
    }

    // --- HELPERS ---

    async fn send_invite_reject(&mut self, target_peer: &PeerId, meeting_id: &str, reason: RejectReason) {
        let reject_payload = InviteRejectPayload {
            meeting_id_hash: meeting_id.to_string(),
            user_id_hash: self.identity.phone_hash.clone(),
            reason_code: match reason {
                RejectReason::MeetingFull => 1,
                RejectReason::FriendsOnlyRestricted => 2,
                _ => 0,
            },
        };

        if let Ok(envelope) = MessageEnvelope::new(
            &self.identity,
            MessageType::InviteReject,
            reject_payload,
            self.db_manager.read().await.storage_radius_km().await.unwrap_or(50),
        ) {
            self.outgoing_messages.push_back((*target_peer, envelope));
            info!("MSG_SENT_INVITE_REJECT: To {} for Meeting {}", target_peer, meeting_id);
        }
    }

    // --- FORWARDING & SENDING ---

    async fn forward_message(&mut self, target_peer: PeerId, packet: MessageEnvelope) {
        self.outgoing_messages.push_back((target_peer, packet));
    }

    /// Drains outgoing messages queue.
    pub fn poll_outgoing(&mut self) -> Option<(PeerId, MessageEnvelope)> {
        self.outgoing_messages.pop_front()
    }

    /// Drains events queue for UI.
    pub fn poll_events(&mut self) -> Option<P2PEvent> {
        self.pending_events.pop_front()
    }

    // --- MANAGEMENT ---

    pub fn add_trusted_peer(&mut self, peer_id: PeerId, user_hash: String) {
        self.discovery.add_trusted_peer(peer_id, user_hash.clone());
        futures::executor::block_on(async {
            self.trust_graph.write().await.add_verified(peer_id, user_hash);
        });
    }

    pub fn register_invite_token(&mut self, token_hash: String, expiry: u64) {
        self.discovery.register_invite_token(token_hash, expiry);
        // Note: ConnectionGuard uses TrustGraph, so adding a guest to TrustGraph 
        // (done after token validation in app logic) is what allows connection.
        // This method just registers token for Discovery layer handshake.
    }

    pub fn set_mode(&mut self, mode: NodeMode) {
        self.mode = mode;
        let mut config = self.discovery.config.clone();
        config.mode = mode;
        self.discovery.update_config(config);
        
        futures::executor::block_on(async {
            self.sync_manager.write().await.set_mode(mode);
        });
    }
}
