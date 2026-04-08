// mobile/rust-core/src/p2p/protocol.rs
// P2P Protocol Definition: Private Mesh Architecture.
// Features: Direct Invites, Chain-of-Trust Forwarding, Compact Statuses.
// Security: No Public Gossip for Meetings (Free Tier), Signature Verification.
// Year: 2026 | Rust Edition: 2024

use serde::{Serialize, Deserialize};
use bincode;
use lz4_flex;
use crate::crypto::identity::Identity;
use crate::dict::cts_parser::CtsTag;
use log::{warn, info};
use chrono::Utc;

/// Current protocol version.
const PROTOCOL_VERSION: u16 = 1;
const MIN_COMPATIBLE_VERSION: u16 = 1;

// -----------------------------------------------------------------------------
// COMPACT ENUMS (Optimized for Network Transmission - 1 Byte)
// -----------------------------------------------------------------------------

/// Status uczestnictwa w spotkaniu.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum ParticipantStatus {
    Invited = 0,      // Zaproszony (oczekuje na akceptację)
    Interested = 1,   // "MOŻE" - lokalne (rzadko przesyłane)
    Confirmed = 2,    // "UCZESTNICZĘ" - publiczne zobowiązanie
    Present = 3,      // Potwierdzony na miejscu
    NoShow = 4,       // Nieobecny mimo potwierdzenia
}

/// Supported Push Providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PushProvider {
    UnifiedPush,
    FCM,
    APNs,
    None,
}

/// Priority levels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PriorityLevel {
    High,
    Normal,
    Low,
}

// -----------------------------------------------------------------------------
// DATA STRUCTURES
// -----------------------------------------------------------------------------

/// Filter criteria for notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationFilter {
    pub level: PriorityLevel,
    pub specific_user_hashes: Vec<String>,
    pub allowed_tags: Vec<String>,
    pub max_distance_km: Option<f32>,
}

/// Header common to all P2P messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    pub version: u16,
    pub msg_type: MessageType,
    pub timestamp: u64,
    pub sender_id_hash: String,
    pub sender_storage_radius_km: u32,
    pub signature: Vec<u8>,
}

/// Types of messages exchanged in the Spotka network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    Handshake,
    // Gossip, // DEPRECATED for meetings in Free Tier. Kept for generic system announcements if needed.
    SyncRequest,
    SyncResponse,
    Ping,
    Pong,
    PushRegister,
    Report,
    DictSync,
    
    // --- NEW: Private Mesh Meeting Types ---
    ParticipationUpdate,
    Invite,         // Zaproszenie na spotkanie (może być forwardowane)
    InviteAccept,   // Akceptacja zaproszenia (do organizatora)
    InviteReject,   // Odrzucenie zaproszenia
}

/// Payload for the Handshake message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakePayload {
    pub trust_anchor_count: u32,
    pub storage_radius_km: u32,
    pub push_provider: PushProvider,
    pub push_token: Option<String>,
    pub supported_languages: Vec<String>,
    pub session_dictionary: Option<Vec<u8>>,
}

/// Payload for Gossip messages (Legacy / System-wide only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipPayload {
    pub meetup_id_hash: String,
    pub organizer_id_hash: String,
    pub location_lat: f64,
    pub location_lon: f64,
    pub start_time: u64,
    pub tags: Vec<CtsTag>,
    pub filter: NotificationFilter,
    pub hop_count: u8,
}

/// NEW: Payload for INVITE messages.
/// Crucial Field: `is_friends_only` controls forwarding logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitePayload {
    pub meeting_id_hash: String,
    pub organizer_id_hash: String, // Who created the meeting
    pub sender_id_hash: String,    // Who is sending this specific packet (forwarder)
    pub recipient_id_hash: String, // Target user
    
    pub token: String,             // Unique invite token (UUID)
    pub created_at: u64,           // Timestamp for expiration check
    
    // Control Flags
    pub is_friends_only: bool,     // TRUE = Do not forward beyond direct friends of Organizer
    pub max_participants: Option<u32>, // Capacity limit
    pub current_guest_count: u32,  // Current confirmed count
    
    // Metadata
    pub location_lat: f64,
    pub location_lon: f64,
    pub start_time: u64,
    pub tags: Vec<CtsTag>,
    
    // Security
    pub organizer_signature: Vec<u8>, // Signature by Organizer proving validity
}

/// NEW: Payload for accepting an invite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteAcceptPayload {
    pub meeting_id_hash: String,
    pub user_id_hash: String,
    pub recipient_id_hash: String, // Usually the Organizer
    pub token: String,             // The token being accepted
    pub user_signature: Vec<u8>,   // User signs to confirm attendance
}

/// NEW: Payload for rejecting an invite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteRejectPayload {
    pub meeting_id_hash: String,
    pub user_id_hash: String,
    pub reason_code: u8, // 0: Declined, 1: Full, 2: Expired, 3: FriendsOnlyRestricted
}

/// Payload for updating participation status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipationUpdatePayload {
    pub meeting_id_hash: String,
    pub user_id_hash: String,
    pub status: ParticipantStatus,
    pub timestamp: u64,
    pub user_signature: Vec<u8>,
}

/// Payload for Push Registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRegisterPayload {
    pub provider: PushProvider,
    pub token: String,
}

/// Payload for Reputation Reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPayload {
    pub target_id_hash: String,
    pub report_type: ReportType,
    pub evidence_hash: String,
    pub reporter_signature: Vec<u8>,
    pub quorum_signatures: Vec<QuorumSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumSignature {
    pub witness_id_hash: String,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportType {
    NoShow,
    FakeProfile,
    Aggression,
    Spam,
}

/// Payload for Dictionary Synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictSyncPayload {
    pub base_version: u64,
    pub patch_data: Vec<u8>,
    pub version: u64,
}

/// The complete Envelope containing header and compressed payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub header: MessageHeader,
    pub payload_bytes: Vec<u8>,
    pub is_compressed: bool,
}

impl MessageEnvelope {
    /// Creates a new envelope, automatically compressing payload if beneficial.
    pub fn new(
        identity: &Identity,
        msg_type: MessageType,
        payload: impl Serialize,
        storage_radius_km: u32,
    ) -> Result<Self, &'static str> {
        let payload_bytes_raw = bincode::serialize(&payload)
            .map_err(|_| "ERR_SERIALIZE_FAILED")?;

        // Compress if payload > 128 bytes
        let (final_bytes, is_compressed) = if payload_bytes_raw.len() > 128 {
            (lz4_flex::compress(&payload_bytes_raw), true)
        } else {
            (payload_bytes_raw, false)
        };

        let now = Utc::now().timestamp() as u64;
        let mut header = MessageHeader {
            version: PROTOCOL_VERSION,
            msg_type,
            timestamp: now,
            sender_id_hash: identity.phone_hash.clone(),
            sender_storage_radius_km: storage_radius_km,
            signature: vec![],
        };

        // Sign: Hash(Header + Payload)
        let mut hasher = sha2::Sha256::new();
        let header_for_hash = MessageHeader {
            signature: vec![],
            ..header.clone()
        };
        
        hasher.update(bincode::serialize(&header_for_hash).unwrap_or_default());
        hasher.update(&final_bytes);
        let digest = hasher.finalize();

        header.signature = identity.sign(&digest).to_bytes().to_vec();

        Ok(MessageEnvelope {
            header,
            payload_bytes: final_bytes,
            is_compressed,
        })
    }

    /// Pre-validation to check basic structural integrity.
    pub fn pre_validate(&self) -> Result<(), &'static str> {
        if self.header.signature.len() != 64 {
            return Err("ERR_INVALID_SIGNATURE_LENGTH");
        }
        if self.header.version < MIN_COMPATIBLE_VERSION || self.header.version > PROTOCOL_VERSION {
            return Err("ERR_PROTOCOL_VERSION_MISMATCH");
        }
        Ok(())
    }

    /// Verifies the signature and integrity of the message.
    pub fn verify(&self, sender_public_key: &[u8]) -> Result<(), &'static str> {
        self.pre_validate()?;

        let now = Utc::now().timestamp() as u64;
        if now - self.header.timestamp > 86400 {
            return Err("ERR_MESSAGE_EXPIRED");
        }

        let mut hasher = sha2::Sha256::new();
        let header_without_sig = MessageHeader {
            signature: vec![],
            ..self.header.clone()
        };
        
        hasher.update(bincode::serialize(&header_without_sig).unwrap_or_default());
        hasher.update(&self.payload_bytes);
        let digest = hasher.finalize();

        let sig = ed25519_dalek::Signature::from_slice(&self.header.signature)
            .map_err(|_| "ERR_INVALID_SIGNATURE_FORMAT")?;

        let pub_key = ed25519_dalek::VerifyingKey::from_bytes(sender_public_key)
            .map_err(|_| "ERR_INVALID_PUBLIC_KEY")?;

        pub_key.verify(&digest, &sig)
            .map_err(|_| "ERR_SIGNATURE_VERIFICATION_FAILED")?;

        Ok(())
    }

    /// Decompresses and deserializes the payload into type T.
    pub fn get_payload<T: for<'de> Deserialize<'de>>(&self) -> Result<T, &'static str> {
        let decompressed = if self.is_compressed {
            lz4_flex::decompress(&self.payload_bytes, 1024 * 1024)
                .map_err(|_| "ERR_DECOMPRESSION_FAILED")?
        } else {
            self.payload_bytes.clone()
        };

        bincode::deserialize(&decompressed)
            .map_err(|_| "ERR_DESERIALIZE_FAILED")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::identity::Identity;

    #[test]
    fn test_invite_payload_structure() {
        let identity = Identity::generate("organizer");
        
        let payload = InvitePayload {
            meeting_id_hash: "m1".to_string(),
            organizer_id_hash: identity.phone_hash.clone(),
            sender_id_hash: identity.phone_hash.clone(),
            recipient_id_hash: "friend".to_string(),
            token: "token123".to_string(),
            created_at: Utc::now().timestamp() as u64,
            is_friends_only: true, // Testing the new flag
            max_participants: Some(10),
            current_guest_count: 2,
            location_lat: 52.0,
            location_lon: 21.0,
            start_time: 1234567890,
            tags: vec![],
            organizer_signature: vec![0u8; 64],
        };

        let raw = bincode::serialize(&payload).unwrap();
        // Ensure serialization works
        assert!(raw.len() > 0);

        let envelope = MessageEnvelope::new(&identity, MessageType::Invite, payload, 60).unwrap();
        assert!(envelope.verify(&identity.verifying_key().to_bytes()).is_ok());

        let decoded: InvitePayload = envelope.get_payload().unwrap();
        assert!(decoded.is_friends_only);
        assert_eq!(decoded.max_participants, Some(10));
    }

    #[test]
    fn test_status_enum_size() {
        assert_eq!(std::mem::size_of::<ParticipantStatus>(), 1);
    }
}

// Placeholder for Ping/Pong payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PingPayload {}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PongPayload {}
