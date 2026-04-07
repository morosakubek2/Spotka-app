// mobile/rust-core/src/ffi/android.rs
// Android FFI Layer: JNI Bindings for Spotka Core.
// Architecture: Zero-Copy where possible, Safe String Handling, Global JVM Context.
// Year: 2026 | Rust Edition: 2024

use jni::objects::{JClass, JString, JObject, JValue, JByteArray};
use jni::sys::{jint, jobject, jstring, jboolean, jbyteArray, JNI_TRUE, JNI_FALSE};
use jni::JNIEnv;
use std::sync::OnceLock;
use log::{info, error, warn};
use android_logger::Config;

// --- Global JVM Context ---
// Allows Rust to call back into Java/Kotlin (e.g., updating UI from P2P thread)
static JVM: OnceLock<JavaVM> = OnceLock::new();

fn get_jvm() -> &'static JavaVM {
    JVM.get().expect("JVM not initialized. Call init_spotka first.")
}

// --- Helper Functions for Safe JNI ---

/// Converts Java String to Rust String. Returns error message key on failure.
fn jstring_to_rust(env: &JNIEnv, obj: JString) -> Result<String, &'static str> {
    env.get_string(&obj)
        .map(|s| s.into())
        .map_err(|_| "ERR_JNI_STRING_CONVERSION_FAILED")
}

/// Converts Rust String to Java String.
fn rust_to_jstring(env: &JNIEnv, s: &str) -> Result<JString<'_>, &'static str> {
    env.new_string(s)
        .map_err(|_| "ERR_JNI_NEW_STRING_FAILED")
}

/// Converts Java ByteArray to Rust Vec<u8>.
fn jbytearray_to_rust(env: &JNIEnv, obj: JByteArray) -> Result<Vec<u8>, &'static str> {
    env.convert_byte_array(obj)
        .map_err(|_| "ERR_JNI_BYTE_ARRAY_CONVERSION_FAILED")
}

// --- Initialization ---

#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_initSpotka(
    mut env: JNIEnv,
    _class: JClass,
    context: JObject,
) {
    // 1. Initialize Logger (Android Logcat)
    android_logger::init_once(
        Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("SpotkaCore"),
    );

    info!("MSG_ANDROID_FFI_INIT_START");

    // 2. Store Global JVM reference
    let vm = env.get_java_vm().unwrap();
    if JVM.set(vm).is_err() {
        error!("ERR_JVM_ALREADY_INITIALIZED");
        return;
    }

    // 3. Initialize Rust Core (DB, Crypto, etc.)
    // This calls the main init function from lib.rs
    crate::spotka_init();
    
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
        Err(e) => return rust_to_jstring(&env, e).unwrap().into_raw(),
    };

    let identity = crate::crypto::identity::Identity::generate(&phone);
    let public_key_hex = hex::encode(identity.verifying_key().as_bytes());
    
    // Return Public Key as Hex String to Java
    match rust_to_jstring(&env, &public_key_hex) {
        Ok(s) => s.into_raw(),
        Err(e) => rust_to_jstring(&env, e).unwrap().into_raw(),
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

    // Async runtime handling would be needed here for non-blocking IO
    // For now, simplified synchronous call
    match tokio::runtime::Runtime::new() {
        Ok(rt) => {
            rt.block_on(async {
                match crate::db::manager::DbManager::new(&path, &token).await {
                    Ok(_) => JNI_TRUE,
                    Err(_) => JNI_FALSE,
                }
            })
        },
        Err(_) => JNI_FALSE,
    }
}

// --- P2P Bridges ---

#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_startP2PNode(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    info!("MSG_ANDROID_P2P_START_REQUEST");
    
    // Spawn P2P node in background
    std::thread::spawn(|| {
        // Logic to start libp2p swarm
        // crate::p2p::start_node().await...
    });

    rust_to_jstring(&env, "MSG_P2P_STARTED").unwrap().into_raw()
}

// --- Callback Mechanism (Example) ---
// Called by Rust internal logic to update UI (e.g., new message received)
pub fn notify_ui_event(event_name: &str, data: &str) {
    let vm = get_jvm();
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(_) => return, // Cannot attach, skip notification
    };

    // Find the SpotkaCore class and call a static method 'onEvent'
    // This requires the Java side to implement: public static void onEvent(String name, String data)
    let class = env.find_class("com/spotka/SpotkaCore").unwrap();
    let j_event = env.new_string(event_name).unwrap();
    let j_data = env.new_string(data).unwrap();

    if let Err(e) = env.call_static_method(
        class,
        "onEvent",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(j_event.into()), JValue::Object(j_data.into())],
    ) {
        error!("ERR_UI_CALLBACK_FAILED: {:?}", e);
    }
}
