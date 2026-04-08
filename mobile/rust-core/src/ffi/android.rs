// mobile/rust-core/src/ffi/android.rs
// Android FFI Layer: JNI Bindings for Spotka Core (Private Mesh Edition).
// Architecture: Zero-Copy where possible, Safe String Handling, Global JVM Context, Async Runtime.
// Security: Memory safe conversions, Error keys instead of messages, No raw key exposure.
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
use crate::app_controller::AppController; // Assumed to exist with updated methods

// --- Global State ---

static JVM: OnceLock<JavaVM> = OnceLock::new();
static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();
static APP_CONTROLLER: OnceLock<Arc<AppController>> = OnceLock::new();

fn get_jvm() -> &'static JavaVM {
    JVM.get().expect("JVM not initialized. Call initSpotka first.")
}

fn get_runtime() -> &'static Arc<Runtime> {
    RUNTIME.get().expect("Runtime not initialized.")
}

fn get_controller() -> &'static Arc<AppController> {
    APP_CONTROLLER.get().expect("Controller not initialized.")
}

// --- Helper Functions for Safe JNI ---

fn jstring_to_rust(env: &JNIEnv, obj: JString) -> Result<String, &'static str> {
    env.get_string(&obj)
        .map(|s| s.into())
        .map_err(|_| "ERR_JNI_STRING_CONVERSION_FAILED")
}

fn rust_to_jstring<'local>(env: &JNIEnv<'local>, s: &str) -> Result<JString<'local>, &'static str> {
    env.new_string(s)
        .map_err(|_| "ERR_JNI_NEW_STRING_FAILED")
}

fn jbytearray_to_rust(env: &JNIEnv, obj: JByteArray) -> Result<Vec<u8>, &'static str> {
    env.convert_byte_array(obj)
        .map_err(|_| "ERR_JNI_BYTE_ARRAY_CONVERSION_FAILED")
}

fn rust_to_jbytearray<'local>(env: &JNIEnv<'local>, data: &[u8]) -> Result<JByteArray<'local>, &'static str> {
    env.byte_array_from_slice(data)
        .map_err(|_| "ERR_JNI_BYTE_ARRAY_CREATION_FAILED")
}

fn return_error_key<'local>(env: &JNIEnv<'local>, key: &str) -> jstring {
    match rust_to_jstring(env, key) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
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

    android_logger::init_once(
        Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("SpotkaCore"),
    );

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

    let runtime = Arc::new(
        Runtime::new().expect("Failed to create Tokio runtime")
    );
    if RUNTIME.set(runtime).is_err() {
        error!("ERR_RUNTIME_ALREADY_INITIALIZED");
        return;
    }

    // Initialize Controller with dependencies (DB, Identity, etc.)
    // In a real app, paths and config would come from 'context'
    let controller = AppController::new(); 
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
            let pub_key_hex = hex::encode(identity.verifying_key().to_bytes());
            match rust_to_jstring(&env, &pub_key_hex) {
                Ok(s) => s.into_raw(),
                Err(e) => return_error_key(&env, e),
            }
        },
        Err(e) => return_error_key(&env, "ERR_IDENTITY_GENERATION_FAILED"),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_signData(
    mut env: JNIEnv,
    _class: JClass,
    private_key_seed: JByteArray, 
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

    // Reconstruct key and sign
    let seed_arr: [u8; 32] = match seed.try_into() {
        Ok(arr) => arr,
        Err(_) => {
            error!("ERR_INVALID_SEED_LENGTH");
            return std::ptr::null_mut();
        }
    };
    
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed_arr);
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
    
    rt.block_on(async {
        match DbManager::new(&path, &token).await {
            Ok(_manager) => {
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
    let controller = get_controller().clone();

    rt.spawn(async move {
        if let Err(e) = controller.start_p2p().await {
            error!("ERR_P2P_START_FAILED: {}", e);
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

// --- NEW: Meeting & Invite Bridges (Private Mesh) ---

/// Creates a meeting with optional "Friends Only" restriction.
#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_createMeeting(
    mut env: JNIEnv,
    _class: JClass,
    meeting_data_json: JString, // Contains lat, lon, time, tags, etc.
    is_friends_only: jboolean,
    max_participants: jint,
) -> jstring {
    let json = match jstring_to_rust(&env, meeting_data_json) {
        Ok(j) => j,
        Err(e) => return return_error_key(&env, e),
    };

    let friends_only = is_friends_only == JNI_TRUE;
    let max_part = if max_participants > 0 { Some(max_participants as u32) } else { None };

    let rt = get_runtime();
    let controller = get_controller().clone();

    // Spawn async task
    rt.spawn(async move {
        match controller.create_meeting(&json, friends_only, max_part).await {
            Ok(meeting_id) => {
                info!("MSG_MEETING_CREATED: {}", meeting_id);
                notify_ui_event("MEETING_CREATED", &meeting_id);
            },
            Err(e) => {
                error!("ERR_CREATE_MEETING_FAILED: {}", e);
                notify_ui_event("MEETING_ERROR", &e.to_string());
            }
        }
    });

    match rust_to_jstring(&env, "MSG_MEETING_CREATING_ASYNC") {
        Ok(s) => s.into_raw(),
        Err(e) => return_error_key(&env, e),
    }
}

/// Sends an invite to a specific user (by Phone Hash or User ID).
#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_sendInvite(
    mut env: JNIEnv,
    _class: JClass,
    meeting_id: JString,
    target_user_hash: JString,
) -> jstring {
    let m_id = match jstring_to_rust(&env, meeting_id) {
        Ok(s) => s,
        Err(e) => return return_error_key(&env, e),
    };
    let t_hash = match jstring_to_rust(&env, target_user_hash) {
        Ok(s) => s,
        Err(e) => return return_error_key(&env, e),
    };

    let rt = get_runtime();
    let controller = get_controller().clone();

    rt.spawn(async move {
        match controller.send_invite(&m_id, &t_hash).await {
            Ok(_) => {
                info!("MSG_INVITE_SENT: {} -> {}", m_id, t_hash);
                notify_ui_event("INVITE_SENT_OK", &m_id);
            },
            Err(e) => {
                error!("ERR_SEND_INVITE_FAILED: {}", e);
                // Specific error codes for UI handling
                let err_code = if e.contains("NOT_IN_NETWORK") { "ERR_TARGET_NOT_IN_NETWORK" } 
                               else if e.contains("FRIENDS_ONLY") { "ERR_FRIENDS_ONLY_RESTRICTED" }
                               else { "ERR_INVITE_SEND_FAILED" };
                notify_ui_event("INVITE_ERROR", err_code);
            }
        }
    });

    match rust_to_jstring(&env, "MSG_INVITE_SENDING_ASYNC") {
        Ok(s) => s.into_raw(),
        Err(e) => return_error_key(&env, e),
    }
}

/// Accepts an invite.
#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_acceptInvite(
    mut env: JNIEnv,
    _class: JClass,
    meeting_id: JString,
    token: JString,
) -> jstring {
    let m_id = match jstring_to_rust(&env, meeting_id) {
        Ok(s) => s,
        Err(e) => return return_error_key(&env, e),
    };
    let tok = match jstring_to_rust(&env, token) {
        Ok(s) => s,
        Err(e) => return return_error_key(&env, e),
    };

    let rt = get_runtime();
    let controller = get_controller().clone();

    rt.spawn(async move {
        match controller.accept_invite(&m_id, &tok).await {
            Ok(_) => {
                info!("MSG_INVITE_ACCEPTED: {}", m_id);
                notify_ui_event("INVITE_ACCEPTED_OK", &m_id);
            },
            Err(e) => {
                error!("ERR_ACCEPT_INVITE_FAILED: {}", e);
                let err_code = if e.contains("FULL") { "ERR_MEETING_FULL" } 
                               else { "ERR_INVITE_INVALID" };
                notify_ui_event("INVITE_ERROR", err_code);
            }
        }
    });

    match rust_to_jstring(&env, "MSG_INVITE_ACCEPTING_ASYNC") {
        Ok(s) => s.into_raw(),
        Err(e) => return_error_key(&env, e),
    }
}

/// Rejects an invite.
#[no_mangle]
pub extern "system" fn Java_com_spotka_SpotkaCore_rejectInvite(
    mut env: JNIEnv,
    _class: JClass,
    meeting_id: JString,
    token: JString,
    reason_code: jint, // 0: Declined, 1: Full, 2: Expired
) -> jstring {
    let m_id = match jstring_to_rust(&env, meeting_id) {
        Ok(s) => s,
        Err(e) => return return_error_key(&env, e),
    };
    let tok = match jstring_to_rust(&env, token) {
        Ok(s) => s,
        Err(e) => return return_error_key(&env, e),
    };

    let rt = get_runtime();
    let controller = get_controller().clone();

    rt.spawn(async move {
        match controller.reject_invite(&m_id, &tok, reason_code as u8).await {
            Ok(_) => {
                info!("MSG_INVITE_REJECTED: {}", m_id);
                notify_ui_event("INVITE_REJECTED_OK", &m_id);
            },
            Err(e) => {
                error!("ERR_REJECT_INVITE_FAILED: {}", e);
                notify_ui_event("INVITE_ERROR", &e.to_string());
            }
        }
    });

    match rust_to_jstring(&env, "MSG_INVITE_REJECTING_ASYNC") {
        Ok(s) => s.into_raw(),
        Err(e) => return_error_key(&env, e),
    }
}

// --- Callback Mechanism (Event Bus) ---

pub fn notify_ui_event(event_name: &str, data: &str) {
    let vm = match get_jvm().attach_current_thread() {
        Ok(e) => e,
        Err(e) => {
            error!("ERR_JVM_ATTACH_FAILED: {:?}", e);
            return;
        }
    };

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

    info!("MSG_ANDROID_FFI_SHUTDOWN_COMPLETE");
}
