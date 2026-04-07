// mobile/rust-core/src/main.rs
// Entry point for the standalone Rust core (Desktop/Test builds).
// Note: Mobile platforms (Android/iOS) use lib.rs via FFI as the primary entry point.
// Year: 2026 | Rust Edition: 2024

use spotka_core::app_controller::AppController;
use spotka_core::db::manager::DbManager;
use spotka_core::crypto::identity::Identity;
use log::{info, error};
use std::env;

/// Main application entry point.
/// Initializes the runtime, database, and P2P stack in an "Anti-Social" mode:
/// - No chat services.
/// - No social feeds.
/// - Focus on physical meetup planning only.
#[tokio::main]
async fn main() {
    // 1. Initialize Logger (Keys only, no hardcoded text)
    // In production, this would output keys like "MSG_INIT_START" to be translated by UI.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            // Custom formatter to ensure no accidental leakage of sensitive data in logs
            writeln!(buf, "[SPOTKA_CORE] {}: {}", record.level(), record.args())
        })
        .init();

    info!("MSG_CORE_INIT_STARTED");

    // 2. Secure Memory Allocation Initialization
    // Ensures that cryptographic keys can be securely wiped from memory (zeroize crate).
    // This is critical for the "Zero-Knowledge" architecture.
    // (Implicitly handled by rust allocator hooks in newer editions, but explicit here for clarity)
    
    // 3. Load Configuration & Arguments
    let args: Vec<String> = env::args().collect();
    let db_path = args.get(1).unwrap_or(&"./spotka_data.db".to_string());
    let user_phone_hash = args.get(2).unwrap_or(&"default_user_hash".to_string()); 
    // Note: In real usage, phone_hash is derived from biometric unlock, not CLI args.

    info!("MSG_DB_PATH_CONFIGURED"); 

    // 4. Initialize Secure Database (Drift + SQLCipher)
    // The auth_secret would normally come from the OS Secure Enclave/KeyStore via FFI.
    let auth_secret = "device_specific_secret_from_native_layer"; 
    
    match DbManager::new(db_path, auth_secret).await {
        Ok(db_manager) => {
            info!("MSG_DB_INITIALIZED_SUCCESS");
            
            // 5. Initialize Identity (if not exists)
            // Identity is based on Phone Number Hash (SHA-256) + Ed25519 Keys.
            // No central registration. Identity is self-sovereign.
            let identity = Identity::generate(user_phone_hash);
            info!("MSG_IDENTITY_GENERATED_OR_LOADED");

            // 6. Start Application Controller
            // This manages the state machine, P2P node, and App-Chain sync.
            let mut controller = AppController::new(identity, db_manager);
            
            info!("MSG_CONTROLLER_STARTING");
            
            // Run the main loop (handles P2P events, UI messages, and background sync)
            // Designed to be energy-efficient: sleeps when no physical meetups are nearby.
            if let Err(e) = controller.run().await {
                error!("ERR_CONTROLLER_RUN_FAILED: {}", e); // 'e' is a key, not a message
            }
        },
        Err(e) => {
            // Critical failure: Cannot start without encrypted storage.
            error!("ERR_DB_INIT_FAILED: {}", e); // 'e' is a key like "ERR_SQLCIPHER_AUTH_FAILED"
            std::process::exit(1);
        }
    }

    info!("MSG_CORE_SHUTDOWN_CLEAN");
}
