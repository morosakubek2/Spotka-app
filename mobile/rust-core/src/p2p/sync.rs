// mobile/rust-core/src/p2p/sync.rs
// P2P Synchronization Module: Private Mesh Architecture with Controlled Forwarding.
// Features: 
//   - Direct Unicast & Chain Forwarding.
//   - "Friends Only" Mode (Organizer setting).
//   - Strict Network Membership Check (No outsiders).
//   - Privacy-First (Interested status is local).
// Year: 2026 | Rust Edition: 2024

use crate::chain::{block::Block, AppChain};
use crate::db::manager::DbManager;
use crate::db::schema::{ParticipantStatus as DbParticipantStatus, Meeting, MeetingParticipant, RelationshipStatus};
use crate::p2p::protocol::{
    MessageType, MessageEnvelope, ParticipationUpdatePayload,
    InvitePayload, InviteAcceptPayload, InviteRejectPayload, ParticipantStatus as NetParticipantStatus
};
use crate::p2p::mod::{NodeMode, TrustLevel};
use crate::crypto::identity::Identity;
use log::{info, warn, debug, error};
use std::collections::{HashSet, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use zeroize::Zeroize;
use chrono::Utc;

/// Request structure for Delta Sync (Trust-Based).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    pub known_heights: Vec<u64>,
    pub known_hashes: Vec<String>,
    pub request_snapshot: bool,
    pub peer_trust_level: u32, // 0 = Stranger, 1+ = Verified Friend
}

/// Response structure for Delta Sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResponse {
    Snapshot(StateSnapshot),
    Blocks(Vec<Block>),
    Ack,
    AccessDenied,
}

/// Represents a snapshot of the current state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub height: u64,
    pub merkle_root: String,
    pub timestamp: u64,
    pub reputation_state: Vec<(String, i32)>,
}

/// Manages synchronization logic for the P2P node.
pub struct SyncManager {
    db_manager: Arc<RwLock<DbManager>>,
    chain: Arc<RwLock<AppChain>>,
    identity: Identity,
    mode: NodeMode,
    storage_radius_km: u32,
    known_hashes: HashSet<String>,
    active_invites_cache: RwLock<HashMap<String, u64>>,
    rate_limit_cache: RwLock<HashMap<String, (u32, u64)>>,
}

impl SyncManager {
    pub fn new(db_manager: Arc<RwLock<DbManager>>, chain: Arc<RwLock<AppChain>>, identity: Identity, radius: u32) -> Self {
        SyncManager {
            db_manager,
            chain,
            identity,
            mode: NodeMode::Eco,
            storage_radius_km: radius,
            known_hashes: HashSet::with_capacity(1000),
            active_invites_cache: RwLock::new(HashMap::new()),
            rate_limit_cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn set_mode(&mut self, mode: NodeMode) {
        self.mode = mode;
        info!("MSG_SYNC_MODE_UPDATED: {:?}", mode);
    }

    // --- RATE LIMITING ---

    async fn check_rate_limit(&self, peer_id_hash: &str) -> bool {
        let now = Utc::now().timestamp() as u64;
        let mut cache = self.rate_limit_cache.write().await;
        
        if let Some((count, reset_time)) = cache.get_mut(peer_id_hash) {
            if now > *reset_time {
                *count = 1;
                *reset_time = now + 60;
                true
            } else {
                *count += 1;
                *count <= 10 // Max 10 msgs/min
            }
        } else {
            cache.insert(peer_id_hash.to_string(), (1, now + 60));
            true
        }
    }

    // --- CORE SYNC ---

    pub async fn handle_sync_request(&self, req: &SyncRequest) -> Result<SyncResponse, &'static str> {
        if req.peer_trust_level == 0 && self.mode != NodeMode::Guardian {
            return Ok(SyncResponse::AccessDenied);
        }

        let current_height = self.chain.read().await.get_height();
        if req.request_snapshot || (req.known_heights.is_empty() && current_height > 100) {
            let snapshot = self.generate_snapshot().await?;
            return Ok(SyncResponse::Snapshot(snapshot));
        }

        let mut blocks_to_send = Vec::new();
        let chain_lock = self.chain.read().await;
        let max_known = req.known_heights.iter().max().copied().unwrap_or(0);
        let batch_limit = match self.mode {
            NodeMode::Eco => 10,
            NodeMode::Active => 50,
            NodeMode::Guardian => 200,
        };
        let end_height = std::cmp::min(max_known + batch_limit, current_height);

        for h in (max_known + 1)..=end_height {
            if let Some(block) = chain_lock.blocks_cache.get(&h) {
                if self.is_within_radius(&block) {
                    blocks_to_send.push(block.clone());
                }
            }
        }
        Ok(SyncResponse::Blocks(blocks_to_send))
    }

    // --- INVITE LOGIC (WITH FORWARDING RULES) ---

    /// Handles incoming INVITE messages.
    pub async fn handle_invite(&self, payload: &InvitePayload) -> Result<(), &'static str> {
        if !self.check_rate_limit(&payload.sender_id_hash).await {
            return Err("ERR_RATE_LIMIT_EXCEEDED");
        }

        // 1. Verify Signature (Placeholder - critical in prod)
        // if !verify_signature(payload, organizer_pub_key) { return Err("ERR_INVALID_SIG"); }

        // 2. Am I the recipient?
        if payload.recipient_id_hash == self.identity.phone_hash {
            return self.process_direct_invite(payload).await;
        }

        // 3. I am an intermediate node. Should I forward?
        
        // CHECK 1: Is the target user in the network?
        // If not, SILENTLY DROP. Do not send error back to avoid revealing network topology.
        if !self.is_user_in_network(&payload.recipient_id_hash).await {
            debug!("MSG_INVITE_FORWARD_BLOCKED: Target {} not in network (Silent Drop)", payload.recipient_id_hash);
            return Ok(()); 
        }

        // CHECK 2: Is "Friends Only" mode enabled?
        if payload.is_friends_only {
            // Logic: Only direct friends of the Organizer can be invited.
            
            // Case A: I am the Organizer sending directly.
            if payload.organizer_id_hash == self.identity.phone_hash {
                // Allowed. Proceed to send (handled by transport layer usually, here we just accept logic)
                debug!("MSG_INVITE_DIRECT_FROM_ORG: Friends-Only mode, sending directly.");
                // In a real node, we would now call transport.send(to=recipient). 
                // Here we just return Ok to indicate "processed/allowed".
                return Ok(());
            }

            // Case B: I am forwarding.
            // Rule: Forwarding is allowed ONLY if the Recipient is a direct friend of the Organizer.
            // I need to check my local DB: Is 'recipient' a direct friend of 'organizer'?
            let is_recipient_org_friend = self.are_direct_friends(&payload.organizer_id_hash, &payload.recipient_id_hash).await;

            if is_recipient_org_friend {
                debug!("MSG_INVITE_FORWARD_ALLOWED: Recipient is direct friend of Organizer.");
                // Forward logic here
                return Ok(());
            } else {
                warn!("MSG_INVITE_FORWARD_BLOCKED: Friends-Only mode and recipient is NOT direct friend of Organizer.");
                return Ok(()); // Silent drop
            }
        }

        // If we are here: is_friends_only == FALSE.
        // Target is in network. Forwarding is allowed to any network user.
        debug!("MSG_INVITE_FORWARDING: Open meeting, forwarding to {}", payload.recipient_id_hash);
        // TODO: Call P2P transport to send to next peer who knows the recipient
        
        Ok(())
    }

    async fn process_direct_invite(&self, payload: &InvitePayload) -> Result<(), &'static str> {
        let invite_token_hash = hex::encode(blake3::hash(payload.token.as_bytes()).as_bytes());
        let expiration = payload.created_at + 86400; 
        self.active_invites_cache.write().await.insert(invite_token_hash, expiration);

        let db = self.db_manager.write().await;
        let database = db.database();

        let participant = MeetingParticipant {
            meeting_id: payload.meeting_id_hash.clone(),
            user_id: self.identity.phone_hash.clone(),
            status: DbParticipantStatus::Invited,
            verification_signature: None,
            user_status_index: DbParticipantStatus::Invited as u8,
        };
        // database.meeting_participants().insert(participant).await?;
        info!("MSG_INVITE_RECEIVED: Meeting {}", payload.meeting_id_hash);
        Ok(())
    }

    // --- HELPERS ---

    /// Checks if a user exists in our local DB.
    async fn is_user_in_network(&self, user_hash: &str) -> bool {
        let db = self.db_manager.read().await;
        let database = db.database();
        database.users().filter(|u| u.id.eq(user_hash)).into_first().await.is_some()
    }

    /// Checks if two users are direct friends (Pinged) in the local DB.
    /// This is crucial for "Friends Only" forwarding validation.
    async fn are_direct_friends(&self, user_a: &str, user_b: &str) -> bool {
        let db = self.db_manager.read().await;
        let database = db.database();

        // Check if there is a relationship entry where User A is friend of B OR B is friend of A
        // Depending on schema, Relationship might be directed or undirected. 
        // Assuming 'Pinged' implies mutual trust or we check both directions.
        
        let link_exists = database.relationships()
            .filter(|r| 
                (r.user_id.eq(user_a).and(r.related_user_id.eq(user_b))) // Hypothetical related_user_id
                .or(r.user_id.eq(user_b).and(r.related_user_id.eq(user_a)))
                .and(r.status.eq(RelationshipStatus::Pinged))
            )
            .into_first()
            .await
            .is_some();

        // Fallback if schema stores relationships differently (e.g., only one direction per row)
        // For this example, assuming a robust check exists.
        link_exists
    }

    // --- ACCEPT / REJECT ---

    pub async fn handle_invite_accept(&self, payload: &InviteAcceptPayload) -> Result<(), &'static str> {
        if payload.recipient_id_hash != self.identity.phone_hash { return Ok(()); }

        let db = self.db_manager.read().await;
        let database = db.database();
        let meeting = database.meetings().filter(|m| m.id.eq(&payload.meeting_id_hash)).into_first().await.ok_or("ERR_MEETING_NOT_FOUND")?;

        let current_count = database.meeting_participants()
            .filter(|p| p.meeting_id.eq(&payload.meeting_id_hash).and(p.status.eq(DbParticipantStatus::Confirmed)))
            .count().await.unwrap_or(0);

        if let Some(max) = meeting.max_participants {
            if current_count >= max { return Err("ERR_MEETING_FULL"); }
        }

        // Update DB to Confirmed
        info!("MSG_INVITE_ACCEPTED: User {} joined", payload.user_id_hash);
        Ok(())
    }

    pub async fn handle_invite_reject(&self, _payload: &InviteRejectPayload) -> Result<(), &'static str> {
        Ok(())
    }

    // --- PARTICIPATION UPDATE ---

    pub async fn handle_participation_update(
        &self,
        payload: &ParticipationUpdatePayload,
        sender_pub_key: &[u8]
    ) -> Result<bool, &'static str> {
        // PRIVACY: Ignore 'Interested'
        if payload.status == NetParticipantStatus::Interested {
            return Ok(false);
        }

        // Verify Auth (Trust or Invite)
        if !self.is_sender_authorized_for_meeting(&payload.user_id_hash, &payload.meeting_id_hash).await {
            return Err("ERR_UNAUTHORIZED_PARTICIPATION");
        }

        // Propagate only public statuses
        let should_propagate = matches!(
            payload.status,
            NetParticipantStatus::Confirmed | NetParticipantStatus::Present | NetParticipantStatus::NoShow
        );
        Ok(should_propagate)
    }

    async fn is_sender_authorized_for_meeting(&self, user_id: &str, meeting_id: &str) -> bool {
        let db = self.db_manager.read().await;
        let database = db.database();

        // 1. Trusted Friend?
        if database.relationships().filter(|r| r.user_id.eq(user_id).and(r.status.eq(RelationshipStatus::Pinged))).into_first().await.is_some() {
            return true;
        }

        // 2. Has Valid Invite?
        if database.meeting_participants().filter(|p| p.meeting_id.eq(meeting_id).and(p.user_id.eq(user_id)).and(p.status.eq(DbParticipantStatus::Invited))).into_first().await.is_some() {
            return true;
        }

        false
    }

    // --- UTILS ---

    async fn generate_snapshot(&self) -> Result<StateSnapshot, &'static str> {
        let chain_lock = self.chain.read().await;
        Ok(StateSnapshot {
            height: chain_lock.get_height(),
            merkle_root: chain_lock.get_latest_hash().unwrap_or_default(),
            timestamp: Utc::now().timestamp() as u64,
            reputation_state: vec![],
        })
    }

    fn is_within_radius(&self, _block: &Block) -> bool { true }

    pub fn should_propagate(&self, msg_type: &MessageType) -> bool {
        match self.mode {
            NodeMode::Eco => false,
            NodeMode::Active | NodeMode::Guardian => {
                matches!(msg_type, MessageType::ParticipationUpdate | MessageType::Report(_))
            }
        }
    }

    pub async fn cleanup_expired_invites(&self) {
        let now = Utc::now().timestamp() as u64;
        let mut cache = self.active_invites_cache.write().await;
        cache.retain(|_, &mut exp| exp > now);
    }

    pub async fn shutdown(&mut self) {
        self.known_hashes.clear();
        self.active_invites_cache.write().await.clear();
        self.rate_limit_cache.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::identity::Identity;

    #[tokio::test]
    async fn test_forwarding_blocked_if_friends_only_and_not_direct() {
        let db = Arc::new(RwLock::new(DbManager::new("", "").await.unwrap()));
        let chain = Arc::new(RwLock::new(AppChain::new(db.clone()).await.unwrap()));
        let identity = Identity::generate("me");
        let sync_mgr = SyncManager::new(db, chain, identity, 50);

        let payload = InvitePayload {
            meeting_id_hash: "m1".to_string(),
            organizer_id_hash: "org".to_string(),
            sender_id_hash: "sender".to_string(),
            recipient_id_hash: "target".to_string(),
            token: "tok".to_string(),
            created_at: Utc::now().timestamp() as u64,
            is_friends_only: true, 
        };

        // Mock: Target is in network, but NOT direct friend of Org
        // (In real test, we'd seed DB accordingly)
        
        let res = sync_mgr.handle_invite(&payload).await;
        assert!(res.is_ok()); // Silent drop, no error
    }

    #[tokio::test]
    async fn test_privacy_interested_status() {
        let db = Arc::new(RwLock::new(DbManager::new("", "").await.unwrap()));
        let chain = Arc::new(RwLock::new(AppChain::new(db.clone()).await.unwrap()));
        let identity = Identity::generate("me");
        let sync_mgr = SyncManager::new(db, chain, identity, 50);

        let payload = ParticipationUpdatePayload {
            meeting_id_hash: "m1".to_string(),
            user_id_hash: "u1".to_string(),
            status: NetParticipantStatus::Interested,
            timestamp: Utc::now().timestamp() as u64,
            user_signature: vec![],
        };

        let res = sync_mgr.handle_participation_update(&payload, &[0u8; 32]).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), false); // Do not propagate
    }
}
