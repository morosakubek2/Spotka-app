// mobile/ios/Spotka/AppDelegate.swift
import UIKit
import spotka_core // Import the Rust static library
import Slint        // Import the Slint runtime

@main
class AppDelegate: UIResponder, UIApplicationDelegate {

    var window: UIWindow?
    var slintApp: Optional<MainWindow> = nil // Instance of the Slint UI

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        
        // 1. Initialize Rust Core
        // Calls the FFI function defined in src/ffi/ios.rs
        spotka_init()
        print("[SPOTKA] Rust Core initialized (Version: \(String(cString: spotka_get_version())))")

        // 2. Setup Main Window & Slint UI
        window = UIWindow(frame: UIScreen.main.bounds)
        
        // Create the Slint main window
        do {
            slintApp = try MainWindow()
            
            // Set up a UIView to host the Slint renderer
            let slintView = slintApp!.window()
            let hostingController = UIViewController()
            hostingController.view = slintView
            
            window?.rootViewController = hostingController
            window?.makeKeyAndVisible()
            
            // Pass initial config to Rust (e.g., documents path for DB)
            let docsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].path
            // Call Rust function to set DB path (pseudo-code, needs actual FFI binding)
            // spotka_set_db_path(docsPath)
            
        } catch {
            fatalError("Failed to initialize Slint UI: \(error)")
        }

        // 3. Request Permissions Early (Location for P2P/Geofencing)
        requestLocationPermissions()

        return true
    }

    // Handle Deep Links (spotka://meetup/...)
    func application(_ app: UIApplication, open url: URL, options: [UIApplication.OpenURLOptionsKey : Any] = [:]) -> Bool {
        if url.scheme == "spotka" {
            print("[SPOTKA] Deep link received: \(url.absoluteString)")
            // Forward URL to Rust logic for parsing
            // spotka_handle_deep_link(url.absoluteString)
            return true
        }
        return false
    }

    // Handle Background Fetch (for P2P sync)
    func application(_ application: UIApplication, performFetchWithCompletionHandler completionHandler: @escaping (UIBackgroundFetchResult) -> Void) {
        print("[SPOTKA] Background fetch triggered")
        // Trigger Rust P2P sync logic here
        // spotka_run_background_sync { result in ... }
        completionHandler(.newData)
    }

    // Helper: Request Location Permissions
    private func requestLocationPermissions() {
        // This would typically use CLLocationManager in a real app
        // For now, we assume the Info.plist keys trigger the system dialog automatically when accessed
        print("[SPOTKA] Location permissions requested via Info.plist keys")
    }
    
    func applicationWillResignActive(_ application: UIApplication) {
        // Pause P2P activity or switch to Eco mode
        // spotka_set_node_mode(Eco)
    }

    func applicationDidEnterBackground(_ application: UIApplication) {
        // Start background task for limited P2P sync if allowed
    }

    func applicationWillEnterForeground(_ application: UIApplication) {
        // Resume Active/Guardian mode
        // spotka_set_node_mode(Active)
    }
}
