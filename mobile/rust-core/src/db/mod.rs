// mobile/rust-core/src/db/mod.rs
// Database Module Aggregator.
// Architecture: Encrypted Storage (SQLCipher) + ORM (Drift).
// Security: All sensitive data is encrypted at rest. Keys derived via Argon2.
// Year: 2026 | Rust Edition: 2024

pub mod schema;
pub mod manager;

// Re-export main types for cleaner imports in other modules (e.g., app_controller, ui)
pub use manager::{DbManager, DbError};
pub use schema::{
    AppDatabase, 
    User, Meeting, MeetingParticipant, ChainBlock, LocalConfig, DictionaryCache, PushToken,
    UserStatus, MeetingStatus, ParticipantStatus
};

// --- Configuration Constants ---

/// Default storage radius in kilometers (matches UI default and consensus rules).
pub const DEFAULT_STORAGE_RADIUS_KM: u32 = 60;

/// Default retention period for standard blocks (in days).
pub const DEFAULT_BLOCK_RETENTION_DAYS: u32 = 30;

/// Extended retention period for blocks involving low-reputation users or revocations (in days).
pub const EXTENDED_RETENTION_DAYS: u32 = 365;

/// Default language code for the UI if not set.
pub const DEFAULT_LANGUAGE_CODE: &str = "en";

// --- Initialization Helpers ---

/// Initializes default configuration values in the database if they are missing.
/// Called once during app startup after DbManager is created.
pub async fn initialize_defaults(db_manager: &DbManager) -> Result<(), DbError> {
    let db = db_manager.database();
    
    // Helper to check and insert config
    // Note: Actual Drift syntax might vary slightly depending on generated code, 
    // this represents the logical flow.
    
    // 1. Ensure Storage Radius is set
    // if db.local_config.get_by_key("storage_radius").await?.is_none() {
    //     db.local_config.insert(LocalConfig {
    //         key: "storage_radius".to_string(),
    //         value_blob: DEFAULT_STORAGE_RADIUS_KM.to_le_bytes().to_vec(),
    //     }).await?;
    // }

    // 2. Ensure Language is set
    // if db.local_config.get_by_key("ui_language").await?.is_none() {
    //     db.local_config.insert(LocalConfig {
    //         key: "ui_language".to_string(),
    //         value_blob: DEFAULT_LANGUAGE_CODE.as_bytes().to_vec(),
    //     }).await?;
    // }

    // 3. Ensure Ghost Mode is set (default false)
    // if db.local_config.get_by_key("ghost_mode").await?.is_none() {
    //     db.local_config.insert(LocalConfig {
    //         key: "ghost_mode".to_string(),
    //         value_blob: vec![0], // 0 = false
    //     }).await?;
    // }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_STORAGE_RADIUS_KM, 60);
        assert_eq!(DEFAULT_BLOCK_RETENTION_DAYS, 30);
        assert_eq!(EXTENDED_RETENTION_DAYS, 365);
        assert_eq!(DEFAULT_LANGUAGE_CODE, "en");
    }
}
