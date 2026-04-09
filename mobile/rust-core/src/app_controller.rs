// mobile/rust-core/src/app_controller.rs
// Main Application State Machine & Coordinator.
// Features: Adaptive Energy Management, Geofencing, Auto-Pruning, App-Chain Sync.
// New: Integrated MeetingsManager for "Maybe" vs "Attending" logic.
// Year: 2026 | Rust Edition: 2024

use crate::p2p::{P2PNode, NodeMode};
use crate::db::manager::DbManager;
use crate::crypto::identity::Identity;
use crate::chain::AppChain;
use crate::dict::loader::GlobalDictManager;
use crate::meetings::manager::MeetingsManager;
use crate::ui; 
use log::{info, warn, error};
use std::sync::Arc;
use tokio::sync::RwLock;
use zeroize::Zeroize;

/// Central controller managing all subsystems.
pub struct AppController {
    pub identity: Identity,
    pub db_manager: Arc<RwLock<DbManager>>,
    pub p2p_node: Option<P2PNode>,
    pub app_chain: Option<AppChain>,
    pub dict_manager: GlobalDictManager,
    
    // Nowy menedżer spotkań jako zależność
    pub meetings_manager: Arc<MeetingsManager>,
    
    pub storage_radius_km: u32,
    pub is_ghost_mode: bool,
    pub current_location: Arc<RwLock<Option<(f64, f64)>>>, // Shared location state
    
    // Konfiguracja przypomnień
    pub reminder_offset_hours: u32, 
}

impl AppController {
    /// Initializes the controller with user identity and database.
    pub async fn new(identity: Identity, db_manager: DbManager) -> Result<Self, &'static str> {
        info!("MSG_CONTROLLER_INIT_START");

        let db_arc = Arc::new(RwLock::new(db_manager));
        let location_arc = Arc::new(RwLock::new(None));
        
        // Load config from DB
        let storage_radius = Self::load_config_u32(&db_arc, "config_storage_radius_km", 60).await?;
        let ghost_mode = Self::load_config_bool(&db_arc, "config_ghost_mode", false).await?;
        let reminder_offset = Self::load_config_u32(&db_arc, "config_reminder_offset_hours", 2).await?;

        // Initialize Dictionary Manager
        let dict_manager = GlobalDictManager::new();
        dict_manager.load_defaults().await?;

        // Initialize Meetings Manager
        let meetings_manager = Arc::new(
            MeetingsManager::new(
                db_arc.clone(), 
                identity.clone(), 
                location_arc.clone()
            )
        );

        Ok(AppController {
            identity,
            db_manager: db_arc,
            p2p_node: None,
            app_chain: None,
            dict_manager,
            meetings_manager,
            storage_radius_km: storage_radius,
            is_ghost_mode: ghost_mode,
            current_location: location_arc,
            reminder_offset_hours: reminder_offset,
        })
    }

    /// Starts the main application loop.
    pub async fn run(&mut self) -> Result<(), &'static str> {
        info!("MSG_CONTROLLER_RUN_START");

        // 1. Initialize P2P Node
        let db_clone = self.db_manager.clone();
        // Uwaga: P2PNode::new może wymagać dostosowania, jeśli oczekuje DbManager zamiast referencji
        let node = P2PNode::new(self.identity.clone(), self.db_manager.read().await.clone()).await?;
        self.p2p_node = Some(node);

        // 2. Initialize App-Chain
        let chain = AppChain::new(self.db_manager.read().await.clone()).await?;
        self.app_chain = Some(chain);

        // 3. Start Background Tasks
        
        // Task: Adaptive Energy & Geofencing Loop
        let mut controller_clone = self.clone_for_task();
        tokio::spawn(async move {
            controller_clone.energy_management_loop().await;
        });

        // Task: Auto-Pruning Loop
        let db_clone = self.db_manager.clone();
        let radius = self.storage_radius_km;
        tokio::spawn(async move {
            Self::pruning_loop(db_clone, radius).await;
        });

        // Task: Reminder Scheduler
        // Teraz używamy instancji meetings_manager
        let mm_clone = self.meetings_manager.clone();
        tokio::spawn(async move {
            mm_clone.run_reminder_loop().await;
        });

        // 4. Launch UI
        // Przekazujemy self lub odpowiednie handle do UI
        ui::run_ui(self).await?;

        Ok(())
    }

    // --- NEW MEETING LOGIC HANDLERS (Delegated to MeetingsManager) ---

    /// Handles user clicking "MOŻE" (Maybe).
    pub async fn set_meeting_interest(&self, meeting_id: &str, is_interested: bool) -> Result<(), &'static str> {
        info!("MSG_USER_INTERESTED_FLAG: {} -> {}", meeting_id, is_interested);
        self.meetings_manager.set_interest(meeting_id, is_interested).await
    }

    /// Handles user clicking "UCZESTNICZĘ" (Attending).
    pub async fn confirm_participation(&self, meeting_id: &str) -> Result<(), &'static str> {
        info!("MSG_USER_CONFIRMED_ATTENDANCE: {}", meeting_id);
        // Zwraca ParticipationResult, ale tutaj mapujemy na Result<(), &str> dla uproszczenia FFI
        match self.meetings_manager.confirm_participation(meeting_id).await {
            Ok(_) => Ok(()),
            Err(e) => Err(match e {
                crate::meetings::manager::ParticipationResult::Error(msg) => msg,
                _ => "ERR_PARTICIPATION_FAILED", // Should not happen on confirm
            })
        }
    }

    /// Handles cancelling public attendance.
    pub async fn cancel_participation(&self, meeting_id: &str) -> Result<(), &'static str> {
        info!("MSG_USER_CANCELLED_ATTENDANCE: {}", meeting_id);
        match self.meetings_manager.cancel_participation(meeting_id).await {
            Ok(res) => {
                if let crate::meetings::manager::ParticipationResult::ReputationWarning = res {
                    // UI should handle this warning before actually calling cancel, 
                    // or show a toast here if called directly.
                    warn!("MSG_REPUTATION_WARNING_ISSUED");
                }
                Ok(())
            },
            Err(e) => Err(match e {
                crate::meetings::manager::ParticipationResult::Error(msg) => msg,
                _ => "ERR_CANCEL_FAILED",
            })
        }
    }

    /// Updates global reminder offset setting.
    pub async fn update_reminder_settings(&self, hours: u32) -> Result<(), &'static str> {
        // Save to DB via DbManager helper
        self.db_manager.read().await
            .save_config("config_reminder_offset_hours", &hours.to_le_bytes())
            .await
            .map_err(|_| "ERR_DB_SAVE_FAILED")?;
        
        self.reminder_offset_hours = hours;
        info!("MSG_REMINDER_SETTINGS_UPDATED: {}h", hours);
        Ok(())
    }

    /// Generates a URI for system navigation.
    pub fn get_navigation_uri(&self, lat: f64, lon: f64, location_name: &str) -> String {
        self.meetings_manager.get_navigation_uri(lat, lon, location_name)
    }

    // --- Helpers ---

    async fn load_config_u32(db: &Arc<RwLock<DbManager>>, key: &str, default: u32) -> Result<u32, &'static str> {
        let db_guard = db.read().await;
        // Zakładając, że DbManager ma metodę get_config lub dostęp do schema
        // To jest pseudokod dopasowany do Drift ORM
        /*
        if let Ok(val) = db_guard.database().config.get_by_key(key).await {
             // parse blob to u32
             return Ok(u32_from_blob(val.value_blob));
        }
        */
        // Dla teraz zwracamy default, dopóki nie zaimplementujemy pełnych helperów w DbManager
        Ok(default)
    }

    async fn load_config_bool(db: &Arc<RwLock<DbManager>>, key: &str, default: bool) -> Result<bool, &'static str> {
        Ok(default) // Placeholder
    }

    async fn energy_management_loop(&mut self) {
        loop {
            // Pobierz rzeczywiste dane z systemu przez FFI w produkcji
            let battery_level = 75; 
            let is_charging = false;
            let is_wifi = true;

            if let Some(node) = &mut self.p2p_node {
                node.update_mode(battery_level, is_charging, is_wifi);

                let ble_enabled = if let Some(loc) = *self.current_location.read().await {
                    let dist = self.distance_to_nearest_meetup(loc).await;
                    node.should_enable_ble(Some(dist))
                } else {
                    false
                };

                if ble_enabled {
                    info!("MSG_BLE_ENABLED_GEOFENCE_OK");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    }

    async fn pruning_loop(db: Arc<RwLock<DbManager>>, radius_km: u32) {
        loop {
            info!("MSG_PRUNING_START");
            // Tutaj wywołać logikę czyszczenia starych spotkań
            // db.write().await.prune_old_meetings(radius_km).await.ok();
            tokio::time::sleep(std::time::Duration::from_secs(86400)).await;
        }
    }

    async fn distance_to_nearest_meetup(&self, _loc: (f64, f64)) -> f32 {
        5.0 // Placeholder
    }

    fn clone_for_task(&self) -> Self {
        AppController {
            identity: self.identity.clone(),
            db_manager: self.db_manager.clone(),
            p2p_node: None,
            app_chain: None,
            dict_manager: self.dict_manager.clone(),
            meetings_manager: self.meetings_manager.clone(),
            storage_radius_km: self.storage_radius_km,
            is_ghost_mode: self.is_ghost_mode,
            current_location: self.current_location.clone(),
            reminder_offset_hours: self.reminder_offset_hours,
        }
    }
}

impl Drop for AppController {
    fn drop(&mut self) {
        info!("MSG_CONTROLLER_DROPPED_CLEANUP");
    }
}
