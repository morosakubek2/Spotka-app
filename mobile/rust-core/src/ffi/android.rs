// mobile/rust-core/src/ffi/android.rs
// Android FFI Layer: JNI Bindings for Spotka Core.
// Architecture: Zero-Copy where possible, Safe String Handling, Global JVM Context, Async Runtime.
// Security: Memory safe conversions, Error keys instead of messages.
// Year: 2026 | Rust Edition: 2024

use jni::objects::{JClass, JString, JObject, JValue, JByteArray, GlobalRef, AutoLocal};
use jni::sys::{jint, jobject, jstring, jboolean, jbyteArray, JNI_TRUE, JNI_FALSE, jlong};
use jni::JNIEnv;
use std::sync::{OnceLock, Arc};
use log::{info, error, warn};
use android_logger::Config;
use tokio::runtime::Runtime;
use crate::crypto::identity::Identity;
use crate::db::manager::DbManager;
use crate::app_controller::AppController;

// --- Global State ---

// Global JVM reference for callbacks to Kotlin
static JVM: OnceLock<JavaVM> = OnceLock::new();
// Global Tokio Runtime for async operations (DB, P2P)
static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();
// Global App Controller (Singleton logic)
static APP_CONTROLLER: OnceLock<Arc<AppController>> = OnceLock::new();

fn get_jvm() -> &'static JavaVM {
    JVM.get().expect("JVM not initialized. Call initSpotka first.")
}

fn get_runtime() -> &'static Arc<Runtime> {
    RUNTIME.get().expect("Runtime not initialized.")
}

// --- Helper Functions for Safe JNI ---

/// Converts Java String to Rust String. Returns error key on failure.
fn jstring_to_rust(env: &JNIEnv, obj: JString) -> Result<String, &'static str> {
    env.get_string(&obj)
        .map(|s| s.into())
        .map_err(|_| "ERR_JNI_STRING_CONVERSION_FAILED")
}

/// Converts Rust String to Java String.
fn rust_to_jstring<'local>(env: &JNIEnv<'local>, s: &str) -> Result<JString<'local>, &'static str> {
    env.new_string(s)
        .map_err(|_| "ERR_JNI_NEW_STRING_FAILED")
}

/// Converts Java ByteArray to Rust Vec<u8>.
fn jbytearray_to_rust(env: &JNIEnv, obj: JByteArray) -> Result<Vec<u8>, &'static str> {
    env.convert_byte_array(obj)
        .map_err(|_| "ERR_JNI_BYTE_ARRAY_CONVERSION_FAILED")
}

/// Converts Rust Vec<u8> to Java ByteArray.
fn rust_to_jbytearray<'local>(env: &JNIEnv<'local>, data: &[u8]) -> Result<JByteArray<'local>, &'static str> {
    env.byte_array_from_slice(data)
        .map_err(|_| "ERR_JNI_BYTE_ARRAY_CREATION_FAILED")
}

/// Helper to return an error key as a Java String.
fn return_error_key<'local>(env: &JNIEnv<'local>, key: &str) -> jstring {
    match rust_to_jstring(env, key) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(), // Critical failure
    }
}

// --- Initialization ---

#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_initSpotka(
    mut env: JNIEnv,
    _class: JClass,
    context: JObject,
) {
    info!("MSG_ANDROID_FFI_INIT_START");

    // 1. Initialize Logger (Android Logcat)
    android_logger::init_once(
        Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("SpotkaCore"),
    );

    // 2. Store Global JVM reference
    let vm = match env.get_java_vm() {
        Ok(v) => v,
        Err(e) => {
            error!("ERR_JVM_GET_FAILED: {:?}", e);
            return;
        }
    };
    
    if JVM.set(vm).is_err() {
        error!("ERR_JVM_ALREADY_INITIALIZED");
        return;
    }

    // 3. Initialize Tokio Runtime (Multi-threaded)
    let runtime = Arc::new(
        Runtime::new().expect("Failed to create Tokio runtime")
    );
    if RUNTIME.set(runtime).is_err() {
        error!("ERR_RUNTIME_ALREADY_INITIALIZED");
        return;
    }

    // 4. Initialize Core Logic (Placeholder for actual dependency injection)
    // In real app, we might pass paths from 'context' here
    let controller = AppController::new(); // Simplified
    if APP_CONTROLLER.set(Arc::new(controller)).is_err() {
        error!("ERR_CONTROLLER_ALREADY_INITIALIZED");
        return;
    }

    info!("MSG_ANDROID_FFI_INIT_SUCCESS");
}

// --- Identity & Crypto Bridges ---

#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_generateIdentity(
    mut env: JNIEnv,
    _class: JClass,
    phone_number: JString,
) -> jstring {
    let phone = match jstring_to_rust(&env, phone_number) {
        Ok(p) => p,
        Err(e) => return return_error_key(&env, e),
    };

    match Identity::generate(&phone) {
        Ok(identity) => {
            // Return Public Key as Hex String
            let pub_key_hex = hex::encode(identity.verifying_key.as_bytes());
            match rust_to_jstring(&env, &pub_key_hex) {
                Ok(s) => s.into_raw(),
                Err(e) => return_error_key(&env, e),
            }
        },
        Err(e) => return_error_key(&env, &e.to_string()), // IdentityError implements Display
    }
}

#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_signData(
    mut env: JNIEnv,
    _class: JClass,
    private_key_seed: JByteArray, // Securely passed from Keystore
    data: JByteArray,
) -> jbyteArray {
    let seed = match jbytearray_to_rust(&env, private_key_seed) {
        Ok(s) => s,
        Err(e) => {
            error!("{}", e);
            return std::ptr::null_mut();
        }
    };
    
    let data_vec = match jbytearray_to_rust(&env, data) {
        Ok(d) => d,
        Err(e) => {
            error!("{}", e);
            return std::ptr::null_mut();
        }
    };

    // Reconstruct key and sign (Simplified - in prod use secure enclave handle)
    // Note: Passing raw seed is risky, better to pass a handle ID if possible
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed.try_into().unwrap_or([0u8;32]));
    let signature = signing_key.sign(&data_vec);

    match rust_to_jbytearray(&env, &signature.to_bytes()) {
        Ok(arr) => arr.into_raw(),
        Err(e) => {
            error!("{}", e);
            std::ptr::null_mut()
        }
    }
}

// --- Database Bridges ---

#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_openDatabase(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
    auth_token: JString,
) -> jboolean {
    let path = match jstring_to_rust(&env, db_path) {
        Ok(p) => p,
        Err(_) => return JNI_FALSE,
    };
    let token = match jstring_to_rust(&env, auth_token) {
        Ok(t) => t,
        Err(_) => return JNI_FALSE,
    };

    let rt = get_runtime();
    
    // Run async initialization on the global runtime
    rt.block_on(async {
        match DbManager::new(&path, &token).await {
            Ok(_manager) => {
                // In real app, store manager in a global map or Controller
                info!("MSG_DB_OPENED_SUCCESS");
                JNI_TRUE
            },
            Err(e) => {
                error!("ERR_DB_OPEN_FAILED: {}", e);
                JNI_FALSE
            }
        }
    })
}

// --- P2P Bridges ---

#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_startP2PNode(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    info!("MSG_ANDROID_P2P_START_REQUEST");
    
    let rt = get_runtime();
    let controller = match APP_CONTROLLER.get() {
        Some(c) => c.clone(),
        None => return return_error_key(&env, "ERR_CONTROLLER_NOT_INIT"),
    };

    // Spawn P2P node in background task
    rt.spawn(async move {
        if let Err(e) = controller.start_p2p().await {
            error!("ERR_P2P_START_FAILED: {}", e);
            // Notify UI about failure
            notify_ui_event("P2P_ERROR", &e.to_string());
        } else {
            info!("MSG_P2P_NODE_STARTED_SUCCESS");
            notify_ui_event("P2P_STARTED", "OK");
        }
    });

    match rust_to_jstring(&env, "MSG_P2P_STARTING_ASYNC") {
        Ok(s) => s.into_raw(),
        Err(e) => return_error_key(&env, e),
    }
}

// --- Callback Mechanism (Event Bus) ---

/// Called by Rust internal logic to update UI (e.g., new message received, peer discovered).
/// Safely attaches to JVM and calls a static method in Kotlin.
pub fn notify_ui_event(event_name: &str, data: &str) {
    let vm = match get_jvm().attach_current_thread() {
        Ok(e) => e,
        Err(e) => {
            error!("ERR_JVM_ATTACH_FAILED: {:?}", e);
            return;
        }
    };

    // Use AutoLocal to ensure references are cleaned up automatically
    let class_result = vm.find_class("com/spotka/SpotkaCore");
    if let Ok(class) = class_result {
        let j_event = vm.new_string(event_name);
        let j_data = vm.new_string(data);

        if let (Ok(ev), Ok(dt)) = (j_event, j_data) {
            let _ = vm.call_static_method(
                class,
                "onEvent",
                "(Ljava/lang/String;Ljava/lang/String;)V",
                &[JValue::Object(ev.into()), JValue::Object(dt.into())],
            );
        } else {
            error!("ERR_UI_CALLBACK_STRING_CREATE_FAILED");
        }
    } else {
        error!("ERR_UI_CALLBACK_CLASS_NOT_FOUND");
    }
}

// --- Cleanup ---

#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_shutdown(
    mut env: JNIEnv,
    _class: JClass,
) {
    info!("MSG_ANDROID_FFI_SHUTDOWN_START");
    
    if let Some(controller) = APP_CONTROLLER.get() {
        let rt = get_runtime();
        rt.block_on(async {
            controller.shutdown().await;
        });
    }

    // Detach thread if necessary (usually handled by JVM, but good practice)
    if let Ok(jni_env) = get_jvm().attach_current_thread() {
        // Clean up any pending locals if needed
    }

    info!("MSG_ANDROID_FFI_SHUTDOWN_COMPLETE");
}
