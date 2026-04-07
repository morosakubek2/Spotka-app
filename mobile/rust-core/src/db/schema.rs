// mobile/rust-core/src/db/schema.rs
// Database Schema Definition for Drift ORM & SQLCipher.
// Privacy: All PII (Phone Numbers) stored as SHA-256 Hashes only.
// Year: 2026 | Rust Edition: 2024

use drift::prelude::*;

/// User Profile Table (Local Cache of Web of Trust)
/// Stores identity metadata, reputation, and verifier lists.
#[derive(DataClass, Clone, Debug)]
pub struct User {
    #[primary_key]
    pub id: String, // SHA-256 Hash of Phone Number (Never raw number)
    
    pub display_name: String, // Local alias or name from contacts
    pub reputation_score: i32, // Calculated score (0-100)
    pub trust_level: i32, // 0: Unverified, 1: Local, 2: Trusted (Web of Trust)
    
    pub last_seen: i64, // Timestamp of last activity
    pub public_key_blob: Vec<u8>, // Ed25519 Public Key
    
    // Web of Trust Metrics
    pub verifier_count: i32, // Number of independent verifiers
    pub verifier_list_blob: Vec<u8>, // Serialized list of Verifier PubKeys (for quick lookup)
    
    // Anti-Sybil & Decay
    pub is_sybil_suspect: bool, // Flagged by consensus algorithm
    pub last_activity_decay: i64, // Timestamp for reputation decay calculation
}

/// Meetup Events Table
/// Stores local copies of meetup metadata. Payloads are encrypted/hashed.
#[derive(DataClass, Clone, Debug)]
pub struct Meeting {
    #[primary_key]
    pub id: String, // UUID v4
    
    pub organizer_id: String, // FK to User.id (Hash)
    
    // Location (Geohash or Lat/Lon) - Stored encrypted if private
    pub location_lat: f64,
    pub location_lon: f64,
    pub location_accuracy_meters: i32,
    
    // Time
    pub start_time: i64, // Unix timestamp
    pub min_duration_mins: i32, // Required stay for reputation gain
    
    // Content (CTS Tags)
    pub tags_cts: String, // Compact Tag Sequence (e.g., "kino0alkohol")
    pub tag_indices_blob: Vec<u8>, // Compressed indices if dictionary synced
    
    // Status & Participants
    pub status: i32, // 0: Planned, 1: Active, 2: Completed, 3: Cancelled
    pub guest_count: i32, // Non-app guests (+X)
    pub invited_users_count: i32,
    
    pub created_at: i64,
    pub expires_at: i64, // For auto-pruning
}

/// Meeting Participants Junction Table
/// Tracks attendance and verification status for reputation scoring.
#[derive(DataClass, Clone, Debug)]
pub struct MeetingParticipant {
    #[primary_key]
    pub meeting_id: String,
    #[primary_key]
    pub user_id: String,
    
    pub status: i32, // 0: Invited, 1: Confirmed, 2: Present (Scanned), 3: No-Show
    pub verification_signature: Option<Vec<u8>>, // Signature proving presence
    pub check_in_time: Option<i64>,
    pub check_out_time: Option<i64>,
}

/// App-Chain Blocks Table (Local Ledger)
/// Stores the immutable history of trust transactions.
#[derive(DataClass, Clone, Debug)]
pub struct ChainBlock {
    #[primary_key]
    pub height: i64,
    
    pub prev_hash: String,
    pub merkle_root: String,
    pub timestamp: i64,
    pub validator_id: String, // Hash of Validator's Phone
    pub signature: Vec<u8>,
    
    pub is_pruned: bool, // Flag for soft-deletion (retention policy)
}

/// App-Chain Transactions Table
/// Individual operations within blocks (TrustIssue, RepUpdate, etc.).
#[derive(DataClass, Clone, Debug)]
pub struct ChainTransaction {
    #[primary_key]
    pub id: String,
    
    pub block_height: i64,
    pub tx_type: String, // Enum string: "TrustIssue", "TrustRevoke", etc.
    pub payload_hash: String, // BLAKE3 hash of content
    pub raw_ Vec<u8>, // Minimal data (e.g., target_user_hash, score_delta)
    pub signature: Vec<u8>,
    
    // Retention Logic: Low-rep users' txs kept longer
    pub retention_until: i64, 
}

/// Local Configuration & State
/// Stores user preferences, keys (encrypted), and sync state.
#[derive(DataClass, Clone, Debug)]
pub struct LocalConfig {
    #[primary_key]
    pub key: String, // e.g., "storage_radius_km", "guardian_mode", "language"
    pub value_blob: Vec<u8>, // Serialized value
}

/// Dictionary Entries (For CTS Compression)
/// Stores local frequency indices for tag compression.
#[derive(DataClass, Clone, Debug)]
pub struct DictionaryEntry {
    #[primary_key]
    pub word: String, // The word (e.g., "kino")
    
    pub category: String,
    pub frequency_index: i32, // Dynamic index assigned by usage frequency
    pub is_custom: bool, // False = Official, True = Custom/Peer-synced
    pub last_used: i64, // For LRU eviction of rare tags
}

// Register all tables with Drift
drift_db_table!(
    User, 
    Meeting, 
    MeetingParticipant, 
    ChainBlock, 
    ChainTransaction, 
    LocalConfig, 
    DictionaryEntry
);

/// Main Database Access Object
#[derive(Clone, Debug)]
pub struct AppDatabase {
    pub users: UsersTable,
    pub meetings: MeetingsTable,
    pub participants: MeetingParticipantsTable,
    pub blocks: ChainBlocksTable,
    pub transactions: ChainTransactionsTable,
    pub config: LocalConfigsTable,
    pub dicts: DictionaryEntriesTable,
}
