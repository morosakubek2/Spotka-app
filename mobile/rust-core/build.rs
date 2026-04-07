// mobile/rust-core/build.rs
// Build script for Spotka Core
// Responsible for compiling Slint UI files and preparing resources before Rust compilation.

fn main() {
    // Print build status for CI/CD logs
    println!("cargo:rerun-if-changed=src/ui/");
    println!("cargo:warning=Starting Slint UI compilation for Spotka...");

    // Compile the main window definition.
    // This processes the .slint file and generates the corresponding Rust struct definitions.
    // The UI follows the "Reductive Functionalism" philosophy: 
    // - No external assets (images/fonts) are embedded here.
    // - All text is handled via i18n keys at runtime, not hardcoded here.
    // - Styles are strictly monochrome (black/white) defined in the .slint source.
    
    let ui_input_path = "src/ui/main_window.slint";
    
    // Attempt to compile the UI. If it fails, the build stops with a clear error message.
    slint_build::compile(ui_input_path).expect(
        "ERROR: Failed to compile Slint UI. Check syntax in main_window.slint and included components."
    );

    // Additional configuration for native linking could go here if needed
    // (e.g., linking specific C libraries for SQLCipher on some platforms, 
    // though usually handled by Cargo features).
    
    println!("cargo:warning=Slint UI compilation successful.");
}
