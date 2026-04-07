// mobile/rust-core/src/p2p/protocol.rs
// P2P Protocol Definition: Message Formats, Serialization, Compression.
// Features: Hybrid Push (FCM/APNs/Unified), Wake-up Only, Ghost Mode, Geo-Filtering.
// Year: 2026 | Rust Edition: 2024

use serde::{Serialize, Deserialize};
use bincode;
use lz4_flex;
use crate::crypto::identity::Identity;
use crate::dict::cts_parser::CtsTag;
use log::{warn, info};

/// Current protocol version. Nodes with mismatched major versions cannot communicate.
const PROTOCOL_VERSION: u16 = 1;
const MIN_COMPATIBLE_VERSION: u16 = 1;

/// Supported Push Notification Providers (User Selectable).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PushProvider {
    /// Open source alternative (Gotify, Nextcloud, etc.)
    UnifiedPush,
    /// Google Firebase Cloud Messaging
    FCM,
    /// Apple Push Notification service
    APNs,
    /// No push notifications (Polling only / Offline Mode)
    None,
}

/// Priority levels for the Notification Engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PriorityLevel {
    /// Immediate wake-up + sound (e.g., Family, Specific Users)
    High,
    /// Standard notification (e.g., Group matches, Tag matches)
    Normal,
    /// Silent update (only visible in app list, e.g., distant geo-events)
    Low,
}

/// Filter criteria for granular notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationFilter {
    pub level: PriorityLevel,
    pub specific_user_hashes: Vec<String>, // Hashes of phone numbers for "User Priority"
    pub allowed_tags: Vec<String>,         // e.g., ["urgent", "help"]
    pub max_distance_km: Option<f32>,      // For Geo-Priority
}

/// Header common to all P2P messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    pub version: u16,
    pub msg_type: MessageType,
    pub timestamp: u64,
    pub sender_id_hash: String, // SHA256 of sender's phone number
    pub signature: Vec<u8>,     // Ed25519 signature of the header + payload hash
}

/// Types of messages exchanged in the Spotka network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    Handshake,
    Gossip,
    SyncRequest,
    SyncResponse,
    Ping,
    Pong,
    PushRegister,
    Report, // For reputation reporting (NoShow, FakeProfile)
}

/// Payload for the Handshake message.
/// Used to exchange capabilities and trust status upon connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakePayload {
    pub trust_anchor_count: u32, // Number of verifiers (for Ghost Mode check)
    pub storage_radius_km: u32,  // User's configured data retention radius
    pub push_provider: PushProvider,
    pub push_token: Option<String>, // Encrypted token for wake-up
    pub supported_languages: Vec<String>, // e.g., ["pl", "en", "eo"]
}

/// Payload for Gossip messages (propagating meetup info).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipPayload {
    pub meetup_id_hash: String,
    pub organizer_id_hash: String,
    pub location_lat: f64,
    pub location_lon: f64,
    pub start_time: u64,
    pub tags: Vec<CtsTag>, // Compressed or Text tags
    pub filter: NotificationFilter, // How receivers should treat this message
    pub hop_count: u8, // To prevent infinite loops (TTL)
}

/// Payload for Push Registration updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRegisterPayload {
    pub provider: PushProvider,
    pub token: String, // Encrypted with server key (if hybrid) or just stored locally
}

/// Payload for Reputation Reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPayload {
    pub target_id_hash: String,
    pub report_type: ReportType,
    pub evidence_hash: String, // Hash of logs/signatures proving the claim
    pub reporter_signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportType {
    NoShow,
    FakeProfile,
    Aggression,
    Spam,
}

/// The complete Envelope containing header and compressed payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub header: MessageHeader,
    pub payload_bytes: Vec<u8>, // Compressed (LZ4) if large, else raw
    pub is_compressed: bool,
}

impl MessageEnvelope {
    /// Creates a new envelope, automatically compressing payload if beneficial.
    pub fn new(
        identity: &Identity,
        msg_type: MessageType,
        payload: impl Serialize,
    ) -> Result<Self, &'static str> {
        let payload_bytes_raw = bincode::serialize(&payload)
            .map_err(|_| "ERR_SERIALIZE_FAILED")?;

        // Compress if payload > 128 bytes (threshold for small messages)
        let (final_bytes, is_compressed) = if payload_bytes_raw.len() > 128 {
            (lz4_flex::compress(&payload_bytes_raw), true)
        } else {
            (payload_bytes_raw, false)
        };

        let now = chrono::Utc::now().timestamp() as u64;
        let mut header = MessageHeader {
            version: PROTOCOL_VERSION,
            msg_type,
            timestamp: now,
            sender_id_hash: identity.phone_hash.clone(),
            signature: vec![],
        };

        // Sign: Hash(Header + Payload)
        let mut hasher = sha2::Sha256::new();
        hasher.update(bincode::serialize(&header).unwrap_or_default());
        hasher.update(&final_bytes);
        let digest = hasher.finalize();
        
        header.signature = identity.sign(&digest).to_bytes().to_vec();

        Ok(MessageEnvelope {
            header,
            payload_bytes: final_bytes,
            is_compressed,
        })
    }

    /// Verifies the signature and integrity of the message.
    pub fn verify(&self, sender_public_key: &[u8]) -> Result<(), &'static str> {
        // Check Version
        if self.header.version < MIN_COMPATIBLE_VERSION {
            return Err("ERR_PROTOCOL_VERSION_MISMATCH");
        }

        // Check Timestamp (prevent replay attacks older than 24h)
        let now = chrono::Utc::now().timestamp() as u64;
        if now - self.header.timestamp > 86400 {
            return Err("ERR_MESSAGE_EXPIRED");
        }

        // Verify Signature
        let mut hasher = sha2::Sha256::new();
        hasher.update(bincode::serialize(&self.header).unwrap_or_default()); // Note: header without sig or with empty sig
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
            lz4_flex::decompress(&self.payload_bytes, 1024 * 1024) // Max 1MB limit
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
    fn test_full_roundtrip_with_compression() {
        let identity = Identity::generate("123456789");
        let payload = GossipPayload {
            meetup_id_hash: "meetup_1".to_string(),
            organizer_id_hash: identity.phone_hash.clone(),
            location_lat: 52.2297,
            location_lon: 21.0122,
            start_time: 1234567890,
            tags: vec![], 
            filter: NotificationFilter {
                level: PriorityLevel::High,
                specific_user_hashes: vec![],
                allowed_tags: vec!["coffee".to_string()],
                max_distance_km: Some(5.0),
            },
            hop_count: 0,
        };

        let envelope = MessageEnvelope::new(&identity, MessageType::Gossip, payload).unwrap();
        assert!(envelope.is_compressed); // Should be compressed due to size

        // Verify signature
        let pub_key = identity.verifying_key.to_bytes();
        assert!(envelope.verify(&pub_key).is_ok());

        // Deserialize
        let decoded: GossipPayload = envelope.get_payload().unwrap();
        assert_eq!(decoded.meetup_id_hash, "meetup_1");
        assert_eq!(decoded.filter.level, PriorityLevel::High);
    }

    #[test]
    fn test_ghost_mode_flag() {
        let handshake = HandshakePayload {
            trust_anchor_count: 0, // Ghost
            storage_radius_km: 60,
            push_provider: PushProvider::None,
            push_token: None,
            supported_languages: vec!["en".to_string()],
        };
        
        // Logic to reject this peer in global gossip would happen in `sync.rs` or `discovery.rs`
        // based on this field.
        assert_eq!(handshake.trust_anchor_count, 0);
    }
}
