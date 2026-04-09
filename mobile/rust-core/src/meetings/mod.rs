// mobile/rust-core/src/meetings/mod.rs
// Meetings Module Aggregator.
// Architecture: Privacy-First (Local Interest), Reputation-Aware (Public Participation).
// Features: Reminders, Mini-Map Data, Navigation URIs, Status Management.
// Year: 2026 | Rust Edition: 2024

pub mod manager;

// Re-export main types for cleaner imports in AppController, FFI, and UI logic
pub use manager::{
    MeetingsManager, 
    MeetingDetailsDto, 
    MiniMapData, 
    ParticipationResult
};

// Re-export DB schema types needed for external interaction (e.g., FFI enums)
pub use crate::db::schema::ParticipantStatus;

// --- Configuration Constants ---

/// Default reminder time before a meeting (in hours) if not set by user.
pub const DEFAULT_REMINDER_HOURS: u64 = 2;

/// Threshold (in hours) before a meeting when canceling affects reputation.
pub const REPUTATION_CANCEL_THRESHOLD_HOURS: u64 = 24;

/// Minimum zoom level for mini-map (closest view).
pub const MIN_MAP_ZOOM: u8 = 10;

/// Maximum zoom level for mini-map (farthest view).
pub const MAX_MAP_ZOOM: u8 = 18;

/// Helper function to create a new MeetingsManager instance.
/// Wraps the constructor for easier access from FFI or high-level logic.
pub fn create_manager(
    db_manager: std::sync::Arc<tokio::sync::RwLock<crate::db::manager::DbManager>>,
    identity: crate::crypto::identity::Identity,
) -> MeetingsManager {
    MeetingsManager::new(db_manager, identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_validity() {
        assert!(DEFAULT_REMINDER_HOURS > 0);
        assert!(REPUTATION_CANCEL_THRESHOLD_HOURS >= DEFAULT_REMINDER_HOURS);
        assert!(MIN_MAP_ZOOM < MAX_MAP_ZOOM);
    }
}
