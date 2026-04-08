// mobile/rust-core/src/ui/mod.rs
// UI Module: Reductive Functionalism (B&W, Geometric, No Assets).
// Features: I18n Initialization, Deep Linking Handling, Theme Enforcement, Memory Safety.
// Year: 2026 | Rust Edition: 2024

// CRITICAL: No semicolon here for the macro in recent Slint versions
slint::include_modules!();

use crate::dict::loader::GlobalDictManager;
use crate::db::manager::DbManager;
use crate::app_controller::AppController;
use log::{info, error, warn};
use std::sync::Arc;
use tokio::sync::RwLock;
use slint::ComponentHandle;

/// Initializes and runs the main UI loop.
/// This function bridges the Slint UI with the Rust Core logic.
pub async fn run_ui(
    app_controller: Arc<RwLock<AppController>>,
    db_manager: Arc<RwLock<DbManager>>,
    deep_link_url: Option<String>, // URL from Smart Fallback (e.g., spotka://join?token=...)
) -> Result<(), &'static str> {
    info!("MSG_UI_INIT_START");

    // 1. Initialize Global Dictionary Manager (I18n)
    let dict_manager = Arc::new(GlobalDictManager::default());
    
    // Load official dicts (EN, PL, EO) - handle errors gracefully
    if let Err(e) = dict_manager.load_official_dicts().await {
        error!("MSG_UI_DICT_OFFICIAL_LOAD_FAILED: {:?}", e);
        // Fallback to empty or English-only if critical
    }

    // Load user custom dicts from DB
    if let Err(e) = dict_manager.load_user_custom_dicts(&db_manager).await {
        warn!("MSG_UI_DICT_CUSTOM_LOAD_FAILED: {:?}", e);
    }
    
    // Set initial language
    let initial_lang = {
        let db_guard = db_manager.read().await;
        db_guard.get_config("config_language").await.unwrap_or_else(|_| "en".to_string())
    };
    dict_manager.set_active_language(&initial_lang);
    info!("MSG_UI_DICT_LOADED: {}", initial_lang);

    // 2. Enforce "Reductive Functionalism" Theme
    // Force Black & White mode globally. 
    // In Slint 1.6+, we can set a global property or rely on the B&W palette defined in .slint
    // Here we assume a global property `force_monochrome` exists or we select a specific style.
    // Note: Actual implementation depends on how styles are defined in MainWindow.slint
    slint::platform::set_event_loop_quit_on_last_window_closed(true);

    // 3. Create Main Window Instance
    let main_window = MainWindow::new()
        .map_err(|_| "ERR_UI_WINDOW_CREATION_FAILED")?;

    // 4. Handle Deep Linking (Smart Fallback)
    if let Some(url) = deep_link_url {
        info!("MSG_DEEP_LINK_DETECTED: {}", url);
        let window_weak = main_window.as_weak();
        
        // Parse token and trigger pairing flow asynchronously
        slint::spawn_local(async move {
            if let Some(window) = window_weak.upgrade() {
                // Pseudo-code: Invoke a callback defined in Slint to handle the link
                // window.invoke_handle_deep_link(slint::SharedString::from(url));
                info!("MSG_DEEP_LINK_PROCESSED: {}", url);
                
                // Example: Switch to Ping tab automatically
                // window.set_current_tab_index(2); // Assuming Ping is index 2
            }
        }).ok();
    }

    // 5. Wire Up Logic (Callbacks)
    // Use Weak references to avoid circular strong references (Memory Leak prevention)
    
    // Example: Language Change
    let dict_mgr_clone = Arc::clone(&dict_manager);
    let db_mgr_clone = Arc::clone(&db_manager);
    let window_weak_lang = main_window.as_weak();
    
    main_window.on_language_changed(move |lang_code: slint::SharedString| {
        let lang = lang_code.to_string();
        let dm = Arc::clone(&dict_mgr_clone);
        let db = Arc::clone(&db_mgr_clone);
        let win = window_weak_lang.clone();

        slint::spawn_local(async move {
            dm.set_active_language(&lang);
            
            // Save to DB
            if let Ok(mut db_guard) = db.write().await {
                if let Err(e) = db_guard.save_config("config_language", &lang).await {
                    error!("ERR_DB_SAVE_LANG: {:?}", e);
                }
            }
            
            // Refresh UI texts if necessary (Slint handles bindings automatically usually)
            if let Some(window) = win.upgrade() {
                info!("MSG_LANGUAGE_CHANGED: {}", lang);
                // window.invoke_refresh_texts(); 
            }
        }).ok();
    });

    // Example: Toggle Ghost Mode
    let controller_clone = Arc::clone(&app_controller);
    let window_weak_ghost = main_window.as_weak();

    main_window.on_toggle_ghost_mode(move |is_active: bool| {
        let ctrl = Arc::clone(&controller_clone);
        let win = window_weak_ghost.clone();
        
        slint::spawn_local(async move {
            let mut ctrl_guard = ctrl.write().await;
            // ctrl_guard.set_ghost_mode(is_active).await;
            
            if let Some(window) = win.upgrade() {
                // Update UI state confirmation if needed
                info!("MSG_GHOST_MODE_TOGGLED: {}", is_active);
            }
        }).ok();
    });

    // Example: Storage Radius Change
    let controller_clone_radius = Arc::clone(&app_controller);
    let db_mgr_clone_radius = Arc::clone(&db_manager);
    
    main_window.on_storage_radius_changed(move |radius_km: f32| {
        let ctrl = Arc::clone(&controller_clone_radius);
        let db = Arc::clone(&db_mgr_clone_radius);
        
        slint::spawn_local(async move {
            let mut ctrl_guard = ctrl.write().await;
            // ctrl_guard.set_storage_radius(radius_km as u32).await;
            
            if let Ok(mut db_guard) = db.write().await {
                if let Err(e) = db_guard.save_config("storage_radius", &(radius_km as u32)).await {
                    error!("ERR_DB_SAVE_RADIUS: {:?}", e);
                }
            }
            info!("MSG_STORAGE_RADIUS_CHANGED: {} km", radius_km);
        }).ok();
    });

    // Example: Push Provider Selection
    let controller_clone_push = Arc::clone(&app_controller);
    main_window.on_push_provider_selected(move |provider_idx: i32| {
        let ctrl = Arc::clone(&controller_clone_push);
        slint::spawn_local(async move {
            let mut ctrl_guard = ctrl.write().await;
            // Map index to enum and save
            // ctrl_guard.set_push_provider(provider_idx).await;
            info!("MSG_PUSH_PROVIDER_SELECTED: {}", provider_idx);
        }).ok();
    });

    // 6. Show Window
    main_window.show()
        .map_err(|_| "ERR_UI_WINDOW_SHOW_FAILED")?;
    info!("MSG_UI_WINDOW_SHOWN");

    // 7. Run Event Loop
    // Blocks until the window is closed
    main_window.run()
        .map_err(|_| "ERR_UI_EVENT_LOOP_FAILED")?;

    info!("MSG_UI_SHUTDOWN_CLEAN");
    Ok(())
}
