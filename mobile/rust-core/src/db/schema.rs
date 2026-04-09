// mobile/rust-core/src/db/schema.rs
// Database Schema Definitions using Drift ORM + SQLCipher.
// Architecture: Zero-Knowledge, Encrypted at Rest, Offline-First.
// Update: Support for Forwarded Invites, Guest Limits, and Relationship Types.
// Year: 2026 | Rust Edition: 2024

use drift::prelude::*;
use serde::{Serialize, Deserialize};

// -----------------------------------------------------------------------------
// ENUMS & TYPES (Optimized for Storage & Transmission)
// -----------------------------------------------------------------------------

/// Typ relacji między użytkownikami.
/// Kluczowe rozróżnienie: fizyczny kontakt vs tylko zaproszenie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum RelationshipType {
    None = 0,
    Pinged = 1,       // Fizycznie zweryfikowani znajomi (pełne zaufanie)
    InvitedOnly = 2,  // Tylko wymiana zaproszeń (brak Pingu, ograniczone zaufanie)
    Blocked = 3,      // Czarna lista
}

impl ColumnType for RelationshipType {
    type Intermediate = u8;
    fn convert_to_value(&self) -> Self::Intermediate { *self as u8 }
    fn convert_from_value(value: Self::Intermediate) -> Option<Self> {
        match value {
            0 => Some(RelationshipType::None),
            1 => Some(RelationshipType::Pinged),
            2 => Some(RelationshipType::InvitedOnly),
            3 => Some(RelationshipType::Blocked),
            _ => None,
        }
    }
}

/// Status uczestnictwa w spotkaniu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ParticipantStatus {
    Invited = 0,          // Zaproszony (czeka na odpowiedź)
    Interested = 1,       // "MOŻE" - lokalne zainteresowanie
    Confirmed = 2,        // "UCZESTNICZĘ" - publiczne zobowiązanie
    PendingApproval = 3,  // NOWE: Gość z zaproszenia "drugiej ręki", czeka na zatwierdzenie na miejscu
    Present = 4,          // Faktycznie obecny (po weryfikacji podpisem)
    NoShow = 5,           // Nieobecny mimo potwierdzenia
    Rejected = 6,         // Odrzucono zaproszenie
}

impl ColumnType for ParticipantStatus {
    type Intermediate = u8;
    fn convert_to_value(&self) -> Self::Intermediate { *self as u8 }
    fn convert_from_value(value: Self::Intermediate) -> Option<Self> {
        match value {
            0 => Some(ParticipantStatus::Invited),
            1 => Some(ParticipantStatus::Interested),
            2 => Some(ParticipantStatus::Confirmed),
            3 => Some(ParticipantStatus::PendingApproval),
            4 => Some(ParticipantStatus::Present),
            5 => Some(ParticipantStatus::NoShow),
            6 => Some(ParticipantStatus::Rejected),
            _ => None,
        }
    }
}

/// Status spotkania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum MeetingStatus {
    Active = 1,
    Cancelled = 3,
}

impl ColumnType for MeetingStatus {
    type Intermediate = u8;
    fn convert_to_value(&self) -> Self::Intermediate { *self as u8 }
    fn convert_from_value(value: Self::Intermediate) -> Option<Self> {
        match value {
            1 => Some(MeetingStatus::Active),
            3 => Some(MeetingStatus::Cancelled),
            _ => None,
        }
    }
}

// -----------------------------------------------------------------------------
// 1. USERS TABLE
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct User {
    #[primary_key]
    pub id: String, // SHA256 Hash of Phone Number
    
    pub display_name: String,
    pub reputation_score: i32, 
    pub trust_level: i32,      
    
    pub public_key_blob: Vec<u8>, 
    pub verifier_count: i32,      
    
    pub is_ghost: bool,           
    pub last_seen: i64,           
    
    #[index]
    pub reputation_index: i32,    
}

// -----------------------------------------------------------------------------
// 2. RELATIONSHIPS TABLE (NEW)
// Explicitly tracks the type of connection between users.
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct Relationship {
    #[primary_key]
    pub user_a: String, // Local user hash
    #[primary_key]
    pub user_b: String, // Remote user hash
    
    pub relation_type: RelationshipType, // Pinged vs InvitedOnly
    pub established_at: i64,
    
    // If InvitedOnly, who initiated the connection chain?
    pub introduced_by: Option<String>, // Hash of the mutual friend who forwarded invite
    
    #[index]
    pub relation_type_index: u8,
}

// -----------------------------------------------------------------------------
// 3. MEETINGS TABLE
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct Meeting {
    #[primary_key]
    pub id: String, 
    
    pub organizer_phone_hash: String, 
    pub location_lat: f64,
    pub location_lon: f64,
    pub location_accuracy_meters: f32, 
    
    pub start_time: i64,
    pub min_duration_mins: i32,
    
    pub tags_cts_raw: String,      
    pub tags_cts_compressed: Option<Vec<u8>>, 
    
    pub status: MeetingStatus,
    
    // Capacity Control
    pub max_participants: Option<i32>, // NULL = no limit
    pub guest_count: i32,              // Current confirmed count
    pub invited_users_count: i32,
    
    pub created_at: i64,
    pub updated_at: i64,

    #[index]
    pub geo_time_index_lat: f64, 
    #[index]
    pub geo_time_index_lon: f64,
    #[index]
    pub status_index: u8,
}

// -----------------------------------------------------------------------------
// 4. MEETING PARTICIPANTS
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct MeetingParticipant {
    #[primary_key]
    pub meeting_id: String,
    #[primary_key]
    pub user_id: String, 
    
    pub status: ParticipantStatus, 
    
    // If status is PendingApproval or Present (for non-pinged users), 
    // this signature proves they were physically verified by Organizer.
    pub verification_signature: Option<Vec<u8>>, 
    
    // Who forwarded the invite to this user? (Chain of trust)
    pub forwarded_by: Option<String>, 
    
    #[index]
    pub user_status_index: u8,
}

// -----------------------------------------------------------------------------
// 5. MEETING INVITES (NEW)
// Tracks invites sent to users who are NOT yet in the Relationships table (or are InvitedOnly).
// Allows "Forwarding" logic.
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct MeetingInvite {
    #[primary_key]
    pub id: String, // UUID
    
    pub meeting_id: String,
    pub recipient_hash: String,
    pub sender_hash: String, // Who sent this specific invite (could be organizer or friend)
    
    pub status: i32, // 0: Pending, 1: Accepted, 2: Declined, 3: Expired
    pub created_at: i64,
    pub expires_at: i64,
    
    // Token used to validate the invite without full P2P handshake initially
    pub invite_token_hash: String, 
    
    #[index]
    pub recipient_index: String,
}

// -----------------------------------------------------------------------------
// 6. APP-CHAIN BLOCKS
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct ChainBlock {
    #[primary_key]
    pub height: i64,
    pub prev_hash: String,
    pub merkle_root: String,
    pub timestamp: i64,
    pub validator_id: String, 
    pub signature: Vec<u8>,
    pub transactions_blob: Vec<u8>, 
    pub extended_retention: bool, 
    #[index]
    pub timestamp_index: i64;
}

// -----------------------------------------------------------------------------
// 7. LOCAL CONFIG
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct LocalConfig {
    #[primary_key]
    pub key: String,
    pub value_blob: Vec<u8>,
    pub updated_at: i64,
}

// -----------------------------------------------------------------------------
// 8. DICTIONARY CACHE
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct DictionaryCache {
    #[primary_key]
    pub word: String,
    pub category: String,
    pub frequency_rank: u32,
    pub static_index: Option<u8>, 
    pub dynamic_index: Option<u8>, 
    pub language_code: String,     
    #[index]
    pub lang_freq_index: String, 
}

// -----------------------------------------------------------------------------
// 9. PUSH TOKENS
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct PushToken {
    #[primary_key]
    pub provider: String, 
    pub token_blob: Vec<u8>, 
    pub registered_at: i64,
    pub last_used: i64,
    pub is_active: bool,
}

// -----------------------------------------------------------------------------
// DRIFT DATABASE STRUCTURE
// -----------------------------------------------------------------------------
drift_db_table!(
    User, 
    Relationship,      // NEW
    Meeting, 
    MeetingParticipant, 
    MeetingInvite,     // NEW
    ChainBlock, 
    LocalConfig, 
    DictionaryCache, 
    PushToken
);

#[derive(Clone, Debug)]
pub struct AppDatabase {
    pub users: UsersTable,
    pub relationships: RelationshipsTable,
    pub meetings: MeetingsTable,
    pub participants: MeetingParticipantsTable,
    pub invites: MeetingInvitesTable,
    pub blocks: ChainBlocksTable,
    pub config: LocalConfigsTable,
    pub dictionary: DictionaryCachesTable,
    pub push_tokens: PushTokensTable,
}
