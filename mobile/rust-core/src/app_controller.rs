// mobile/rust-core/src/app_controller.rs
// Main Application State Machine & Coordinator.
// Features: Adaptive Energy Management, Geofencing, Auto-Pruning, App-Chain Sync.
// Year: 2026 | Rust Edition: 2024

use crate::p2p::{P2PNode, NodeMode};
use crate::db::manager::DbManager;
use crate::crypto::identity::Identity;
use crate::chain::AppChain;
use crate::dict::loader::GlobalDictManager;
use crate::ui; // Slint UI integration
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
    pub storage_radius_km: u32,
    pub is_ghost_mode: bool,
    pub current_location: Option<(f64, f64)>, // (lat, lon)
}

impl AppController {
    /// Initializes the controller with user identity and database.
    pub async fn new(identity: Identity, db_manager: DbManager) -> Result<Self, &'static str> {
        info!("MSG_CONTROLLER_INIT_START");

        let db_arc = Arc::new(RwLock::new(db_manager));
        
        // Load config (Storage Radius, Ghost Mode) from DB
        let storage_radius = Self::load_storage_radius(&db_arc).await?;
        let ghost_mode = Self::load_ghost_mode(&db_arc).await?;

        // Initialize Dictionary Manager (Load official + custom dicts)
        let dict_manager = GlobalDictManager::new();
        dict_manager.load_defaults().await?;

        Ok(AppController {
            identity,
            db_manager: db_arc,
            p2p_node: None,
            app_chain: None,
            dict_manager,
            storage_radius_km: storage_radius,
            is_ghost_mode: ghost_mode,
            current_location: None,
        })
    }

    /// Starts the main application loop.
    /// Spawns background tasks for P2P, Chain Sync, and Pruning.
    pub async fn run(&mut self) -> Result<(), &'static str> {
        info!("MSG_CONTROLLER_RUN_START");

        // 1. Initialize P2P Node
        let db_clone = self.db_manager.clone();
        let node = P2PNode::new(self.identity.clone(), db_clone.read().await.clone()).await?;
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

        // Task: Auto-Pruning Loop (Run every 24h)
        let db_clone = self.db_manager.clone();
        let radius = self.storage_radius_km;
        tokio::spawn(async move {
            Self::pruning_loop(db_clone, radius).await;
        });

        // Task: App-Chain Sync Loop
        // (Implementation omitted for brevity, but spawns sync worker)

        // 4. Launch UI (Blocking call usually, or spawned depending on platform)
        // In mobile, UI is launched from Native side calling into Rust.
        // Here we assume desktop/test mode or callback to native.
        ui::run_ui(self).await?;

        Ok(())
    }

    /// Updates operational mode based on battery and location (Geofencing).
    async fn energy_management_loop(&mut self) {
        loop {
            // Simulate getting system stats (in real app, passed from Native via FFI)
            let battery_level = 75; // Placeholder
            let is_charging = false;
            let is_wifi = true;

            if let Some(node) = &mut self.p2p_node {
                // Update Node Mode (Eco/Active/Guardian)
                node.update_mode(battery_level, is_charging, is_wifi);

                // Geofencing Logic for BLE
                let ble_enabled = if let Some(loc) = self.current_location {
                    // Check distance to nearest active meetup
                    let dist = self.distance_to_nearest_meetup(loc).await;
                    node.should_enable_ble(Some(dist))
                } else {
                    false
                };

                if ble_enabled {
                    info!("MSG_BLE_ENABLED_GEOFENCE_OK");
                    // node.enable_ble_advertising();
                } else {
                    // node.disable_ble_advertising();
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    }

    /// Periodically removes old data outside Storage Radius.
    async fn pruning_loop(db: Arc<RwLock<DbManager>>, radius_km: u32) {
        loop {
            info!("MSG_PRUNING_START");
            // db.write().await.prune_old_data(radius_km).await.ok();
            tokio::time::sleep(std::time::Duration::from_secs(86400)).await; // 24h
        }
    }

    /// Handles deep links (e.g., spotka://join?code=XYZ)
    pub async fn handle_deep_link(&self, url: &str) -> Result<(), &'static str> {
        info!("MSG_DEEP_LINK_RECEIVED: {}", url);
        // Parse URL and trigger appropriate action (e.g., join meetup, add friend)
        Ok(())
    }

    // --- Helpers ---

    async fn load_storage_radius(db: &Arc<RwLock<DbManager>>) -> Result<u32, &'static str> {
        // Fetch from DB or return default (60km)
        Ok(60)
    }

    async fn load_ghost_mode(db: &Arc<RwLock<DbManager>>) -> Result<bool, &'static str> {
        // Fetch from DB or return default (false)
        Ok(false)
    }

    async fn distance_to_nearest_meetup(&self, _loc: (f64, f64)) -> f32 {
        // Query DB for meetups and calculate Haversine distance
        5.0 // Placeholder
    }

    // Helper to clone self for task spawning (requires deriving Clone or manual impl)
    fn clone_for_task(&self) -> Self {
        // Simplified for example; real impl needs careful Arc handling
        AppController {
            identity: self.identity.clone(),
            db_manager: self.db_manager.clone(),
            p2p_node: None, // Will be re-initialized or shared via Arc
            app_chain: None,
            dict_manager: self.dict_manager.clone(),
            storage_radius_km: self.storage_radius_km,
            is_ghost_mode: self.is_ghost_mode,
            current_location: self.current_location,
        }
    }
}

// Ensure sensitive fields are zeroed on drop
impl Drop for AppController {
    fn drop(&mut self) {
        // Explicitly zeroize any sensitive buffers if held directly here
        info!("MSG_CONTROLLER_DROPPED_CLEANUP");
    }
}
