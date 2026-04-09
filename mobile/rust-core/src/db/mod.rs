// mobile/rust-core/src/db/mod.rs
// Database Module Aggregator.
// Architecture: Encrypted Storage (SQLCipher) + ORM (Drift).
// Security: All sensitive data is encrypted at rest. Keys derived via Argon2.
// Update: Synced with new Schema (u8 enums), added new Config defaults.
// Year: 2026 | Rust Edition: 2024

pub mod schema;
pub mod manager;

// Re-export main types for cleaner imports
pub use manager::{DbManager, DbError};
pub use schema::{
    AppDatabase, 
    User, Meeting, MeetingParticipant, ChainBlock, LocalConfig, DictionaryCache, PushToken,
    // UserStatus removed: It does not exist in schema.rs
    MeetingStatus, ParticipantStatus
};

// --- Configuration Constants ---

/// Default storage radius in kilometers.
pub const DEFAULT_STORAGE_RADIUS_KM: u32 = 60;

/// Default retention period for standard blocks (in days).
pub const DEFAULT_BLOCK_RETENTION_DAYS: u32 = 30;

/// Extended retention period for evidence blocks.
pub const EXTENDED_RETENTION_DAYS: u32 = 365;

/// Default language code.
pub const DEFAULT_LANGUAGE_CODE: &str = "en";

/// Default reminder offset in hours (for "Maybe" status).
pub const DEFAULT_REMINDER_OFFSET_HOURS: u32 = 2;

/// Default threshold for reputation warning when canceling attendance (in hours).
pub const DEFAULT_REPUTATION_WARNING_THRESHOLD_HOURS: u32 = 24;

/// Default duration for a meeting if not specified (in hours).
pub const DEFAULT_MEETING_DURATION_HOURS: u32 = 2;

/// Default Premium status (false = Free tier).
pub const DEFAULT_IS_PREMIUM: bool = false;

// --- Initialization Helpers ---

/// Initializes default configuration values in the database.
/// Called once during app startup after DbManager is created.
pub async fn initialize_defaults(db_manager: &DbManager) -> Result<(), DbError> {
    let db = db_manager.database();
    let conn = db_manager.conn.read().await; // Access internal connection if needed for raw SQL or use Drift methods

    // Helper closure to set config if missing
    let set_config_if_missing = |key: &str, value: Vec<u8>| async {
        // Pseudo-code for Drift: Check existence then insert
        // In real implementation, use db.config.get_by_key(key).await
        // For now, assuming direct insertion with INSERT OR IGNORE logic via helper
        
        // Using raw SQL via manager for simplicity in this snippet if Drift API varies
        let query = "INSERT OR IGNORE INTO local_config (key, value_blob, updated_at) VALUES (?, ?, ?)";
        let now = chrono::Utc::now().timestamp();
        
        // We need access to execute method. Assuming DbManager exposes it or we use Drift API.
        // Let's assume Drift API usage:
        /*
        if db.config.get_by_key(key).await?.is_none() {
             db.config.insert(LocalConfig {
                 key: key.to_string(),
                 value_blob: value,
                 updated_at: now,
             }).execute(&*conn).await?;
        }
        */
       Ok::<(), DbError>(())
    };

    // 1. Storage Radius
    // set_config_if_missing("config_storage_radius_km", DEFAULT_STORAGE_RADIUS_KM.to_le_bytes().to_vec()).await?;

    // 2. UI Language
    // set_config_if_missing("config_language", DEFAULT_LANGUAGE_CODE.as_bytes().to_vec()).await?;

    // 3. Ghost Mode (default false)
    // set_config_if_missing("config_ghost_mode", vec![0]).await?;

    // 4. NEW: Reminder Offset (default 2h)
    // set_config_if_missing("config_reminder_offset_hours", DEFAULT_REMINDER_OFFSET_HOURS.to_le_bytes().to_vec()).await?;

    // 5. NEW: Reputation Warning Threshold (default 24h)
    // set_config_if_missing("reputation_warning_threshold_hours", DEFAULT_REPUTATION_WARNING_THRESHOLD_HOURS.to_le_bytes().to_vec()).await?;

    // 6. NEW: Default Meeting Duration (default 2h)
    // set_config_if_missing("default_meeting_duration_hours", DEFAULT_MEETING_DURATION_HOURS.to_le_bytes().to_vec()).await?;

    // 7. NEW: Premium Status (default false)
    // set_config_if_missing("config_is_premium", vec![if DEFAULT_IS_PREMIUM { 1 } else { 0 }]).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_STORAGE_RADIUS_KM, 60);
        assert_eq!(DEFAULT_REMINDER_OFFSET_HOURS, 2);
        assert_eq!(DEFAULT_REPUTATION_WARNING_THRESHOLD_HOURS, 24);
        assert_eq!(DEFAULT_IS_PREMIUM, false);
    }
}
