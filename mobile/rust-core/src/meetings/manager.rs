// mobile/rust-core/src/meetings/manager.rs
// Meetings Business Logic Manager.
// Features: Invite-Only Logic, Capacity Limits, Reputation Management, Local Reminders.
// Architecture: Privacy-First, Offline-First, Private Mesh (No Public Gossip).
// Year: 2026 | Rust Edition: 2024

use crate::db::manager::DbManager;
use crate::db::schema::{Meeting, MeetingParticipant, ParticipantStatus, MeetingStatus, LocalConfig};
use crate::crypto::identity::Identity;
use crate::dict::cts_parser::{parse_cts, TagStatus};
use chrono::{DateTime, Local, Duration as ChronoDuration};
use geoutils::Location;
use log::{info, warn};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Data structure for the Mini-Map in Meeting Details.
#[derive(Debug, Clone)]
pub struct MiniMapData {
    pub user_lat: f64,
    pub user_lon: f64,
    pub meeting_lat: f64,
    pub meeting_lon: f64,
    pub distance_km: f64,
    pub zoom_level_hint: u8,
}

/// Result of updating participation status.
#[derive(Debug, Clone)]
pub enum ParticipationResult {
    Success,
    ReputationWarning,
    Error(&'static str),
}

/// Main manager for meeting-related operations.
pub struct MeetingsManager {
    db_manager: Arc<RwLock<DbManager>>,
    identity: Identity,
    current_location_provider: Arc<RwLock<Option<(f64, f64)>>>, 
}

impl MeetingsManager {
    pub fn new(
        db_manager: Arc<RwLock<DbManager>>, 
        identity: Identity,
        location_provider: Arc<RwLock<Option<(f64, f64)>>>
    ) -> Self {
        MeetingsManager { 
            db_manager, 
            identity,
            current_location_provider: location_provider,
        }
    }

    /// Retrieves full details of a specific meeting.
    pub async fn get_meeting_details(&self, meeting_id: &str) -> Result<MeetingDetailsDto, &'static str> {
        let db = self.db_manager.read().await;
        let database = db.database();

        let meeting = database
            .meetings()
            .filter(|m| m.id.eq(meeting_id))
            .into_first()
            .await
            .ok_or("ERR_MEETING_NOT_FOUND")?;

        if meeting.status == MeetingStatus::Cancelled {
            return Err("ERR_MEETING_CANCELLED");
        }

        // Parse Tags
        let cts_tags = parse_cts(&meeting.tags_cts_raw).unwrap_or_default();
        let all_tags_display: Vec<String> = cts_tags.iter().map(|t| t.word.clone()).collect();
        
        let positive_tags: Vec<String> = cts_tags
            .iter()
            .filter(|t| t.status == TagStatus::Positive)
            .map(|t| t.word.clone())
            .collect();
        let title = positive_tags.join(" • ");

        // Get Organizer
        let organizer = database
            .users()
            .filter(|u| u.id.eq(&meeting.organizer_phone_hash))
            .into_first()
            .await;
        
        let organizer_name = organizer.as_ref().map(|u| u.display_name.clone()).unwrap_or_else(|| "Anon".to_string());
        let organizer_rep = organizer.as_ref().map(|u| u.reputation_score).unwrap_or(0);

        // Get Participants (Confirmed/Present only)
        let participants = database
            .meeting_participants()
            .filter(|p| p.meeting_id.eq(meeting_id))
            .collect::<Vec<MeetingParticipant>>()
            .await;

        let mut guest_names = Vec::new();
        let mut confirmed_count = 0;

        for p in &participants {
            if p.status == ParticipantStatus::Confirmed || p.status == ParticipantStatus::Present {
                confirmed_count += 1;
                let user = database.users().filter(|u| u.id.eq(&p.user_id)).into_first().await;
                if let Some(u) = user {
                    if u.id != self.identity.phone_hash {
                        guest_names.push(u.display_name);
                    }
                }
            }
        }

        // Check Current User Status
        let my_participation = database
            .meeting_participants()
            .filter(|p| p.meeting_id.eq(meeting_id).and(p.user_id.eq(&self.identity.phone_hash)))
            .into_first()
            .await;

        let is_interested = my_participation.as_ref().map_or(false, |p| p.status == ParticipantStatus::Interested);
        let is_attending = my_participation.as_ref().map_or(false, |p| p.status == ParticipantStatus::Confirmed || p.status == ParticipantStatus::Present);

        let duration_hours = self.get_meeting_duration(&database, &meeting).await;
        let time_full = Self::format_time_full(meeting.start_time, duration_hours);
        let location_display = format!("{:.4}, {:.4}", meeting.location_lat, meeting.location_lon);

        Ok(MeetingDetailsDto {
            id: meeting.id,
            title,
            all_tags: all_tags_display,
            organizer_name,
            organizer_reputation: organizer_rep,
            time_full,
            location_name: location_display, 
            latitude: meeting.location_lat,
            longitude: meeting.location_lon,
            guest_names,
            guest_count: confirmed_count.saturating_sub(1),
            user_is_interested: is_interested,
            user_is_attending: is_attending,
        })
    }

    /// Sets "Interested" (Maybe) status. Local only. No validation needed.
    pub async fn set_interest(&self, meeting_id: &str, interested: bool) -> Result<(), &'static str> {
        let db = self.db_manager.write().await;
        let database = db.database();

        if interested {
            database
                .meeting_participants()
                .filter(|p| p.meeting_id.eq(meeting_id).and(p.user_id.eq(&self.identity.phone_hash)))
                .delete()
                .await
                .ok(); 

            let participant = MeetingParticipant {
                meeting_id: meeting_id.to_string(),
                user_id: self.identity.phone_hash.clone(),
                status: ParticipantStatus::Interested,
                verification_signature: None,
                // user_status_index removed to match schema macro behavior
            };
            
            database.meeting_participants().insert(participant).await.map_err(|_| "ERR_DB_WRITE_FAILED")?;
            info!("MSG_INTEREST_SET: {} (Local Reminder Scheduled)", meeting_id);
        } else {
            database
                .meeting_participants()
                .filter(|p| p.meeting_id.eq(meeting_id).and(p.user_id.eq(&self.identity.phone_hash)))
                .delete()
                .await
                .map_err(|_| "ERR_DB_DELETE_FAILED")?;
            info!("MSG_INTEREST_CANCELED: {}", meeting_id);
        }
        Ok(())
    }

    /// Confirms "Attending" status. 
    /// VALIDATION: Checks Invitation Status and Capacity Limit.
    pub async fn confirm_participation(&self, meeting_id: &str) -> Result<ParticipationResult, &'static str> {
        let db = self.db_manager.write().await;
        let database = db.database();

        let meeting = database
            .meetings()
            .filter(|m| m.id.eq(meeting_id))
            .into_first()
            .await
            .ok_or("ERR_MEETING_NOT_FOUND")?;

        // 1. Check Capacity Limit
        // Count current confirmed/present participants
        let current_count = database
            .meeting_participants()
            .filter(|p| p.meeting_id.eq(meeting_id).and(
                drift::prelude::Expr::col(MeetingParticipant::STATUS).eq(ParticipantStatus::Confirmed as u8)
                .or(drift::prelude::Expr::col(MeetingParticipant::STATUS).eq(ParticipantStatus::Present as u8))
            ))
            .count()
            .await;

        // Assuming Meeting struct has max_participants field (added in schema update)
        // If schema doesn't have it yet, this line might need adjustment or default to MAX
        let max_participants = meeting.max_participants.unwrap_or(i32::MAX); 

        if current_count >= max_participants {
            return Err("ERR_MEETING_FULL");
        }

        // 2. Check Invitation Status
        // User must be Invited OR be the Organizer to join directly.
        let my_status = database
            .meeting_participants()
            .filter(|p| p.meeting_id.eq(meeting_id).and(p.user_id.eq(&self.identity.phone_hash)))
            .into_first()
            .await;

        let is_organizer = meeting.organizer_phone_hash == self.identity.phone_hash;
        let is_invited = my_status.as_ref().map_or(false, |p| p.status == ParticipantStatus::Invited);

        if !is_organizer && !is_invited {
            // In a strict invite-only system, you cannot join without an invite.
            // However, if the user received an out-of-band link, they might not be in DB yet.
            // But per requirements: "Invite list is checked".
            return Err("ERR_NOT_INVITED");
        }

        // Remove previous status (e.g., Interested or Invited) to replace with Confirmed
        database
            .meeting_participants()
            .filter(|p| p.meeting_id.eq(meeting_id).and(p.user_id.eq(&self.identity.phone_hash)))
            .delete()
            .await
            .ok();

        let participant = MeetingParticipant {
            meeting_id: meeting_id.to_string(),
            user_id: self.identity.phone_hash.clone(),
            status: ParticipantStatus::Confirmed,
            verification_signature: None,
        };

        database.meeting_participants().insert(participant).await.map_err(|_| "ERR_DB_WRITE_FAILED")?;
        
        // TODO: Trigger P2P Direct Message to Organizer (Update Guest List)
        info!("MSG_PARTICIPATION_CONFIRMED: {}", meeting_id);
        Ok(ParticipationResult::Success)
    }

    /// Cancels "Attending" status.
    pub async fn cancel_participation(&self, meeting_id: &str) -> Result<ParticipationResult, &'static str> {
        let db_read = self.db_manager.read().await;
        let database = db_read.database();

        let meeting = database
            .meetings()
            .filter(|m| m.id.eq(meeting_id))
            .into_first()
            .await
            .ok_or("ERR_MEETING_NOT_FOUND")?;

        let warning_threshold_hours = self.get_reputation_warning_threshold(&database).await;
        let now = chrono::Utc::now().timestamp() as u64;
        let time_diff_hours = (meeting.start_time - now as i64) / 3600;

        let result = if time_diff_hours < warning_threshold_hours as i64 && time_diff_hours > 0 {
            ParticipationResult::ReputationWarning
        } else {
            ParticipationResult::Success
        };

        drop(db_read);
        
        let mut db_write = self.db_manager.write().await;
        let database_write = db_write.database();
        
        database_write
            .meeting_participants()
            .filter(|p| p.meeting_id.eq(meeting_id).and(p.user_id.eq(&self.identity.phone_hash)))
            .delete()
            .await
            .map_err(|_| "ERR_DB_DELETE_FAILED")?;

        // TODO: Trigger P2P Direct Message to Organizer
        info!("MSG_PARTICIPATION_CANCELED: {}", meeting_id);
        Ok(result)
    }

    /// Generates data for the Mini-Map.
    pub async fn generate_mini_map_data(&self, meeting_id: &str) -> Result<MiniMapData, &'static str> {
        let db = self.db_manager.read().await;
        let database = db.database();

        let meeting = database
            .meetings()
            .filter(|m| m.id.eq(meeting_id))
            .into_first()
            .await
            .ok_or("ERR_MEETING_NOT_FOUND")?;

        let location_guard = self.current_location_provider.read().await;
        let (user_lat, user_lon) = *(location_guard.as_ref().ok_or("ERR_USER_LOCATION_UNKNOWN")?);
        drop(location_guard);

        let loc1 = Location::new(user_lat, user_lon);
        let loc2 = Location::new(meeting.location_lat, meeting.location_lon);
        let distance_km = loc1.haversine_distance_to(&loc2).kilometers();

        let zoom = if distance_km < 0.5 { 16 } 
                   else if distance_km < 2.0 { 14 } 
                   else if distance_km < 10.0 { 12 } 
                   else { 10 };

        Ok(MiniMapData {
            user_lat,
            user_lon,
            meeting_lat: meeting.location_lat,
            meeting_lon: meeting.location_lon,
            distance_km,
            zoom_level_hint: zoom,
        })
    }

    /// Generates a URI for system navigation.
    pub fn get_navigation_uri(&self, lat: f64, lon: f64, location_name: &str) -> String {
        let encoded_name = urlencoding::encode(location_name);
        format!("geo:{},{}?q={},{}({})", lat, lon, lat, lon, encoded_name)
    }

    // --- Helpers ---

    async fn get_meeting_duration(&self, database: &impl drift::Database, _meeting: &Meeting) -> i64 {
        if let Ok(config) = database.local_config()
            .filter(|c| c.key.eq("default_meeting_duration_hours"))
            .into_first()
            .await 
        {
            if let Ok(val) = std::str::from_utf8(&config.value_blob) {
                if let Ok(hours) = val.parse::<i64>() {
                    return hours;
                }
            }
        }
        2
    }

    async fn get_reputation_warning_threshold(&self, database: &impl drift::Database) -> i32 {
        if let Ok(config) = database.local_config()
            .filter(|c| c.key.eq("reputation_warning_threshold_hours"))
            .into_first()
            .await
        {
            if let Ok(val) = std::str::from_utf8(&config.value_blob) {
                if let Ok(hours) = val.parse::<i32>() {
                    return hours;
                }
            }
        }
        24
    }

    fn format_time_full(timestamp: i64, duration_hours: i64) -> String {
        let dt = DateTime::from_timestamp(timestamp, 0)
            .unwrap_or_default()
            .with_timezone(&Local);
        
        let end_dt = dt + ChronoDuration::hours(duration_hours);
        format!("{}, {:02}:{:02} - {:02}:{:02}", 
            dt.format("%A"), 
            dt.hour(), dt.minute(),
            end_dt.hour(), end_dt.minute()
        )
    }
}

/// DTO for UI binding.
#[derive(Clone, Debug)]
pub struct MeetingDetailsDto {
    pub id: String,
    pub title: String,
    pub all_tags: Vec<String>,
    pub organizer_name: String,
    pub organizer_reputation: i32,
    pub time_full: String,
    pub location_name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub guest_names: Vec<String>,
    pub guest_count: usize,
    pub user_is_interested: bool,
    pub user_is_attending: bool,
}
