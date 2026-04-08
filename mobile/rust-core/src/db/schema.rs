// mobile/rust-core/src/db/schema.rs
// Database Schema Definitions using Drift ORM + SQLCipher.
// Architecture: Zero-Knowledge, Encrypted at Rest, Offline-First.
// Year: 2026 | Rust Edition: 2024

use drift::prelude::*;

// -----------------------------------------------------------------------------
// 1. USERS TABLE (Local Trust Graph Copy)
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct User {
    #[primary_key]
    pub id: String, // SHA256 Hash of Phone Number
    
    pub display_name: String, // From contacts or self-set alias
    pub reputation_score: i32, // -100 to +100
    pub trust_level: i32,      // 0 to 5 (Web of Trust depth)
    
    pub public_key_blob: Vec<u8>, // Ed25519 Public Key
    pub verifier_count: i32,      // How many people vouched for them
    
    // Privacy & Mode Flags
    pub is_ghost: bool,           // True if user requested "Ghost Mode"
    pub last_seen: i64,           // Timestamp
    
    // Indexing helpers
    #[index]
    pub reputation_index: i32,    // Mirror of score for fast sorting
}

// -----------------------------------------------------------------------------
// 2. MEETINGS TABLE (Public Metadata)
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct Meeting {
    #[primary_key]
    pub id: String, // UUID v4
    
    pub organizer_phone_hash: String, // Who created it
    pub location_lat: f64,
    pub location_lon: f64,
    pub location_accuracy_meters: f32, // Fuzzy location precision
    
    pub start_time: i64,
    pub min_duration_mins: i32,
    
    // Tags: Store both raw CTS and compressed version if available
    pub tags_cts_raw: String,      
    pub tags_cts_compressed: Option<Vec<u8>>, // Optional binary blob of indices
    
    pub status: i32, // 0: Planned, 1: Active, 2: Finished, 3: Cancelled
    pub guest_count: i32,
    pub invited_users_count: i32,
    
    pub created_at: i64,
    pub updated_at: i64,

    // Indexes for Geofencing & Time queries
    #[index]
    pub geo_time_index_lat: f64; 
    #[index]
    pub geo_time_index_lon: f64;
    #[index]
    pub status_index: i32;
}

// -----------------------------------------------------------------------------
// 3. MEETING PARTICIPANTS (Junction Table)
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct MeetingParticipant {
    #[primary_key]
    pub meeting_id: String,
    
    #[primary_key]
    pub user_id: String, // Phone Hash
    
    pub status: i32, // 0: Invited, 1: Confirmed, 2: Present, 3: No-Show
    pub verification_signature: Option<Vec<u8>>, // Signature proving presence
    
    #[index]
    pub user_status_index: i32;
}

// -----------------------------------------------------------------------------
// 4. APP-CHAIN BLOCKS (Local Ledger Copy)
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct ChainBlock {
    #[primary_key]
    pub height: i64,
    
    pub prev_hash: String,
    pub merkle_root: String,
    pub timestamp: i64,
    pub validator_id: String, // Phone Hash of Validator
    pub signature: Vec<u8>,
    
    // Serialized transactions (binary) to save space
    pub transactions_blob: Vec<u8>, 
    
    // Retention Flag: Keep longer if involves low-rep users (evidence)
    pub extended_retention: bool, 

    #[index]
    pub timestamp_index: i64;
}

// -----------------------------------------------------------------------------
// 5. LOCAL CONFIG (Key-Value Store)
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct LocalConfig {
    #[primary_key]
    pub key: String,
    
    pub value_blob: Vec<u8>,
    pub updated_at: i64,
}

// Common Keys for LocalConfig:
// - "config_storage_radius_km" (u32)
// - "config_ghost_mode" (bool)
// - "config_language" (String)
// - "sync_vector_local" (Vec<u64> serialized)
// - "dict_version_official" (u32)

// -----------------------------------------------------------------------------
// 6. DICTIONARY CACHE (Dynamic Tag Dictionary)
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct DictionaryCache {
    #[primary_key]
    pub word: String,
    
    pub category: String,
    pub frequency_rank: u32,
    pub static_index: Option<u8>, // If from official dict
    pub dynamic_index: Option<u8>, // If assigned locally
    pub language_code: String,     // e.g., "pl", "en"
    
    #[index]
    pub lang_freq_index: String; // Composite helper
}

// -----------------------------------------------------------------------------
// 7. PUSH TOKENS (For Wake-Up Services)
// -----------------------------------------------------------------------------
#[derive(DataClass, Clone, Debug)]
pub struct PushToken {
    #[primary_key]
    pub provider: String, // "unifiedpush", "fcm", "apns"
    
    pub token_blob: Vec<u8>, // Encrypted token
    pub registered_at: i64,
    pub last_used: i64,
    pub is_active: bool,
}

// -----------------------------------------------------------------------------
// DRIFT DATABASE STRUCTURE
// -----------------------------------------------------------------------------
drift_db_table!(
    User, 
    Meeting, 
    MeetingParticipant, 
    ChainBlock, 
    LocalConfig, 
    DictionaryCache, 
    PushToken
);

#[derive(Clone, Debug)]
pub struct AppDatabase {
    pub users: UsersTable,
    pub meetings: MeetingsTable,
    pub participants: MeetingParticipantsTable,
    pub blocks: ChainBlocksTable,
    pub config: LocalConfigsTable,
    pub dictionary: DictionaryCachesTable,
    pub push_tokens: PushTokensTable,
}
