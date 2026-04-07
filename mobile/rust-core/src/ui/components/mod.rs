// mobile/rust-core/src/ui/components/mod.rs
// UI Components Module: Reusable widgets for the Spotka interface.
// Exports all components and necessary data structures for Slint bindings.
// Year: 2026 | Rust Edition: 2024

// Import generated Slint code (this macro is expanded by build.rs)
slint::include_modules!();

// Re-export main components for easy access in other modules
pub use generated::{
    TagBadge,
    UserCard,
    NavBar,
    MeetingCard,
    // ReputationDisplay is usually a sub-component inside UserCard or MeetingCard, 
    // but if exposed separately:
    // ReputationDisplay, 
};

// --- Data Structures for Bindings ---
// These structs mirror the Slint properties to allow easy data passing from Rust to UI.

/// Represents a single CTS tag for the UI.
#[derive(Clone, Debug)]
pub struct UiTag {
    pub label: slint::SharedString,
    pub status: i32, // Matches TagStatus enum in Slint (0: Positive, 1: Negative, etc.)
    pub is_compacted: bool,
    pub is_official: bool,
}

/// Represents a user summary for lists/cards.
#[derive(Clone, Debug)]
pub struct UiUser {
    pub id: slint::SharedString,
    pub display_name: slint::SharedString,
    pub phone_label: slint::SharedString, // From contacts book
    pub reputation_score: i32, // 0 to 5
    pub verifier_count: i32,
    pub is_verified_by_me: bool,
    pub is_guest: bool, // True if not in app (only for organizer view)
    pub is_ghost_mode: bool,
}

/// Represents a meeting summary for the list/card.
#[derive(Clone, Debug)]
pub struct UiMeeting {
    pub id: slint::SharedString,
    pub main_tag: slint::SharedString,
    pub tags: slint::VecModel<UiTag>, // Vector of tags
    pub organizer_name: slint::SharedString,
    pub organizer_reputation: i32,
    pub distance_text: slint::SharedString, // e.g., "2.3 km"
    pub start_time_text: slint::SharedString, // e.g., "18:30"
    pub duration_text: slint::SharedString, // e.g., "min. 45 min"
    pub participants_text: slint::SharedString, // The "ciurkiem" list
    pub participant_count: i32,
    pub guest_count: i32,
    pub is_premium_event: bool, // Shows venue name if true
    pub venue_name: slint::SharedString, // Only for Premium
    pub status: i32, // 0: Upcoming, 1: Active, 2: Past
}

/// Helper function to convert internal Rust types to Slint types if needed.
pub fn create_tag_model(tags: Vec<UiTag>) -> slint::VecModel<UiTag> {
    slint::VecModel::from_vec(tags)
}

pub fn create_user_model(users: Vec<UiUser>) -> slint::VecModel<UiUser> {
    slint::VecModel::from_vec(users)
}

pub fn create_meeting_model(meetings: Vec<UiMeeting>) -> slint::VecModel<UiMeeting> {
    slint::VecModel::from_vec(meetings)
}
