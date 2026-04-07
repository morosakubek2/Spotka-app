// mobile/rust-core/src/db/mod.rs
// Database Module: Encrypted Storage (SQLCipher) + ORM (Drift).
// Exports: Schema, Manager, Error Types, and Utilities.
// Year: 2026 | Rust Edition: 2024

pub mod schema;
pub mod manager;

// Re-export commonly used types for cleaner imports in other modules
pub use schema::{User, Meeting, MeetingParticipant, ChainBlock, LocalConfig, DictionaryEntry, AppDatabase};
pub use manager::{DbManager, DbError};

/// Database-related constants.
pub mod consts {
    /// Default storage radius in kilometers (matches UI default).
    pub const DEFAULT_STORAGE_RADIUS_KM: u32 = 60;
    
    /// Maximum age of blocks before pruning (in days).
    pub const DEFAULT_BLOCK_RETENTION_DAYS: u32 = 30;
    
    /// Extended retention for low-reputation users (in days).
    pub const EXTENDED_RETENTION_DAYS: u32 = 365;
}

/// Helper function to initialize the database with default config if missing.
/// Called during app startup.
pub async fn init_defaults(db: &DbManager) -> Result<(), DbError> {
    let database = db.database();
    
    // Check if storage radius is set, if not, set default
    let radius_key = "config_storage_radius";
    // Pseudo-code for Drift lookup (actual implementation depends on generated Drift code)
    // if database.config.get_by_key(radius_key).await?.is_none() {
    //     database.config.insert(LocalConfig {
    //         key: radius_key.to_string(),
    //         value_blob: DEFAULT_STORAGE_RADIUS_KM.to_le_bytes().to_vec(),
    //     }).await?;
    // }
    
    Ok(())
}
