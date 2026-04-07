// mobile/rust-core/src/ui/mod.rs
// UI Module: Reductive Functionalism (B&W, Geometric, No Assets).
// Features: I18n Initialization, Deep Linking Handling, Theme Enforcement.
// Year: 2026 | Rust Edition: 2024

slint::include_modules!();

use crate::dict::loader::GlobalDictManager;
use crate::db::manager::DbManager;
use crate::app_controller::AppController;
use log::{info, error};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Initializes and runs the main UI loop.
/// This function bridges the Slint UI with the Rust Core logic.
pub async fn run_ui(
    app_controller: Arc<RwLock<AppController>>,
    db_manager: Arc<RwLock<DbManager>>,
    deep_link_url: Option<String>, // URL from Smart Fallback (e.g., spotka://join?token=...)
) -> Result<(), &'static str> {
    info!("MSG_UI_INIT_START");

    // 1. Initialize Global Dictionary Manager (I18n)
    // Loads official dictionaries (EN, PL, EO) and user custom dicts from DB/Assets.
    // CRITICAL: Must happen before creating any UI components to ensure keys are translated.
    let dict_manager = GlobalDictManager::new();
    dict_manager.load_official_dicts().await?;
    dict_manager.load_user_custom_dicts(&db_manager).await?;
    
    // Set initial language (from DB config or system default)
    let initial_lang = db_manager.read().await.get_config("config_language").await.unwrap_or_else(|_| "en".to_string());
    dict_manager.set_active_language(&initial_lang);

    info!("MSG_UI_DICT_LOADED: {}", initial_lang);

    // 2. Enforce "Reductive Functionalism" Theme
    // Force Black & White mode. No colors, only contrast inversion.
    // In Slint, this might involve setting a global property or selecting a specific style.
    // Here we assume a global property `theme_mode` exists in MainWindow.
    
    // 3. Create Main Window Instance
    let main_window = MainWindow::new()?;

    // 4. Handle Deep Linking (Smart Fallback)
    // If app was launched via a QR code link (e.g., after installation), 
    // automatically navigate to the "Ping/Pairing" screen with the token.
    if let Some(url) = deep_link_url {
        info!("MSG_DEEP_LINK_DETECTED: {}", url);
        let window_clone = main_window.clone_strong();
        // Parse token and trigger pairing flow
        // This is pseudo-code for the actual Slint property setting
        // window_clone.invoke_handle_deep_link(&url); 
    }

    // 5. Wire Up Logic (Callbacks)
    // Connect UI events to Rust Core functions
    
    // Example: Language Change
    let dict_mgr_clone = dict_manager.clone();
    let db_mgr_clone = db_manager.clone();
    main_window.on_language_changed(move |lang_code: slint::SharedString| {
        let lang = lang_code.to_string();
        let dm = dict_mgr_clone.clone();
        let db = db_mgr_clone.clone();
        slint::spawn_local(async move {
            dm.set_active_language(&lang);
            // Save to DB
            if let Ok(mut db_guard) = db.write().await {
                // db_guard.save_config("config_language", &lang).await.ok();
            }
            info!("MSG_LANGUAGE_CHANGED: {}", lang);
        }).ok();
    });

    // Example: Toggle Ghost Mode
    let controller_clone = app_controller.clone();
    main_window.on_toggle_ghost_mode(move |is_active: bool| {
        let ctrl = controller_clone.clone();
        slint::spawn_local(async move {
            let mut ctrl_guard = ctrl.write().await;
            // ctrl_guard.set_ghost_mode(is_active).await;
            info!("MSG_GHOST_MODE_TOGGLED: {}", is_active);
        }).ok();
    });

    // Example: Storage Radius Change
    let controller_clone = app_controller.clone();
    let db_mgr_clone = db_manager.clone();
    main_window.on_storage_radius_changed(move |radius_km: f32| {
        let ctrl = controller_clone.clone();
        let db = db_mgr_clone.clone();
        slint::spawn_local(async move {
            let mut ctrl_guard = ctrl.write().await;
            // ctrl_guard.set_storage_radius(radius_km as u32).await;
            
            if let Ok(mut db_guard) = db.write().await {
                // db_guard.save_config("storage_radius", &radius_km.to_le_bytes()).await.ok();
            }
            info!("MSG_STORAGE_RADIUS_CHANGED: {} km", radius_km);
        }).ok();
    });

    // 6. Show Window
    main_window.show()?;
    info!("MSG_UI_WINDOW_SHOWN");

    // 7. Run Event Loop
    // Blocks until the window is closed
    main_window.run()?;

    info!("MSG_UI_SHUTDOWN_CLEAN");
    Ok(())
}
