// mobile/rust-core/src/main.rs
// Entry point for standalone builds (Desktop/Test environments).
// Note: Mobile platforms (Android/iOS) use lib.rs via FFI as the primary entry point.
// Year: 2026 | Rust Edition: 2024

use spotka_core::app_controller::AppController;
use spotka_core::db::manager::DbManager;
use spotka_core::crypto::identity::Identity;
use spotka_core::dict::loader::GlobalDictManager;
use log::{info, error, warn};
use std::env;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Main application entry point for Desktop/Test builds.
/// Initializes the runtime, database, dictionaries, and P2P stack.
fn main() {
    // 1. Initialize Logger
    // Configurable via RUST_LOG environment variable (e.g., RUST_LOG=debug ./spotka)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            writeln!(buf, "[SPOTKA_CORE] {}: {}", record.level(), record.args())
        })
        .init();

    info!("MSG_CORE_INIT_STARTED");

    // 2. Parse Command Line Arguments
    let args: Vec<String> = env::args().collect();
    
    // Usage: spotka [db_path] [user_phone_hash]
    let db_path = args.get(1).cloned().unwrap_or_else(|| "./spotka_data.db".to_string());
    let user_phone_input = args.get(2).cloned().unwrap_or_else(|| "123456789".to_string()); 
    // Note: In real usage, phone_hash is derived from biometric unlock or secure input.

    info!("MSG_DB_PATH_CONFIGURED: {}", db_path);

    // 3. Create Async Runtime
    let rt = match Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            error!("ERR_RUNTIME_INIT_FAILED: {}", e);
            std::process::exit(1);
        }
    };

    // 4. Run Async Initialization
    let result = rt.block_on(async {
        run_app(db_path, user_phone_input).await
    });

    // 5. Handle Result & Cleanup
    match result {
        Ok(_) => info!("MSG_CORE_SHUTDOWN_CLEAN"),
        Err(e) => {
            error!("ERR_CORE_RUN_FAILED: {}", e);
            std::process::exit(1);
        }
    }
}

/// Asynchronous main logic runner.
async fn run_app(db_path: String, user_phone_input: String) -> Result<(), &'static str> {
    // A. Initialize Secure Database (Drift + SQLCipher)
    // The auth_secret would normally come from OS KeyStore/SecureEnclave.
    // For desktop test, we use a mock or derive from input (NOT for production).
    let auth_secret = "desktop_mock_secret_do_not_use_in_prod"; 
    
    let db_manager = DbManager::new(&db_path, auth_secret).await?;
    info!("MSG_DB_INITIALIZED_SUCCESS");

    // B. Initialize Identity
    // Hash the phone number to create User ID
    let identity = Identity::generate(&user_phone_input);
    info!("MSG_IDENTITY_GENERATED: UID={}", identity.phone_hash);

    // C. Initialize Global Dictionary Manager
    // Loads official dictionaries from assets/dicts/
    let dict_manager = GlobalDictManager::new();
    // In real app, load files from disk here
    info!("MSG_DICT_MANAGER_INIT");

    // D. Start Application Controller
    // This manages the state machine, P2P node, and App-Chain sync.
    let mut controller = AppController::new(identity, db_manager, dict_manager);
    
    info!("MSG_CONTROLLER_STARTING");
    
    // E. Run Main Loop
    // This loop handles P2P events, UI messages, and background sync.
    // It runs until the user quits or a critical error occurs.
    controller.run().await?;

    // F. Explicit Cleanup (Zeroize sensitive data if needed)
    // (Handled by Drop traits usually, but explicit here for clarity)
    
    Ok(())
}
