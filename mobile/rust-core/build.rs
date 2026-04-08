// mobile/rust-core/build.rs
// Build script for Spotka Core.
// Responsibilities: Compile Slint UI, Generate Drift code, Configure Linker.
// Year: 2026 | Rust Edition: 2024

use std::env;
use std::path::PathBuf;
use std::fs;

fn main() {
    // 1. Compile Slint UI Files
    // We must explicitly list all .slint files to ensure cargo re-runs this script on change.
    
    let ui_root = PathBuf::from("src/ui");
    
    // Main window
    let main_window = ui_root.join("main_window.slint");
    println!("cargo:rerun-if-changed={}", main_window.display());
    
    slint_build::compile(&main_window).expect("Failed to compile main_window.slint");

    // Components (Iterate through components folder if it exists)
    let components_dir = ui_root.join("components");
    if components_dir.exists() {
        if let Ok(entries) = fs::read_dir(&components_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("slint") {
                    println!("cargo:rerun-if-changed={}", path.display());
                    // Compile components individually or rely on main_window importing them.
                    // Usually, compiling the root file that imports others is enough, 
                    // but listing them ensures rerun triggers.
                    // slint_build::compile(&path).ok(); // Optional if imported by main
                }
            }
        }
    }

    // 2. Drift Code Generation (if using build-time generation macros)
    // Note: Modern Drift often uses runtime macros or separate CLI tools (drift-cli).
    // If using 'drift' crate with build.rs integration:
    // drift_build::generate_schema("src/db/schema.rs").expect("Failed to generate DB schema");
    // For now, assuming runtime macro usage which doesn't strictly require build.rs steps 
    // other than standard rust compilation.

    // 3. Linker Instructions for Native Targets
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    match target_os.as_str() {
        "android" => {
            // Android NDK usually handles linking, but sometimes explicit logs help
            println!("cargo:warning=Building for Android...");
        },
        "ios" => {
            // iOS requires linking specific frameworks if not handled by Cargo dependencies
            println!("cargo:rustc-link-lib=framework=Security");
            println!("cargo:rustc-link-lib=framework=SystemConfiguration");
            println!("cargo:warning=Building for iOS...");
        },
        "linux" => {
            // For local desktop testing, ensure sqlite3 is linked
            // pkg-config might be needed: println!("cargo:rustc-link-lib=sqlite3");
        },
        _ => {}
    }

    // 4. General Rerun Triggers
    // Ensure build script runs if any source file changes (broad trigger for safety)
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=Cargo.toml");
}
