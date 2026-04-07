// mobile/ios/Spotka/SceneDelegate.swift
// Manages the UI lifecycle, Slint Backend initialization, and Deep Linking.
// Year: 2026 | Swift 5.9+

import UIKit
import Slint
// Import the generated Rust library header
import spotka_core 

class SceneDelegate: UIResponder, UIWindowSceneDelegate {

    var window: UIWindow?
    // Keep a strong reference to the Slint window to prevent it from being deallocated
    private var slintWindow: SlintWindow?

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
        
        guard let windowScene = (scene as? UIWindowScene) else { return }

        // 1. Initialize Slint Backend for iOS
        // This MUST be called before creating any Slint windows.
        // It sets up the event loop and rendering context (CoreGraphics/Metal).
        do {
            try SlintBackend.shared.initIOS()
        } catch {
            fatalError("Failed to initialize Slint backend: \(error)")
        }

        // 2. Create the Main Window from Rust
        do {
            // 'MainWindow' is the component exported from main_window.slint
            self.slintWindow = try MainWindow()
            
            // Optional: Set initial data or callbacks here if needed immediately
            // self.slintWindow?.set_global_tr... 
        } catch {
            fatalError("Failed to load Slint MainWindow: \(error)")
        }

        // 3. Setup Native UIWindow
        let window = UIWindow(windowScene: windowScene)
        
        // 4. Embed Slint View into UIKit
        if let slintWindow = self.slintWindow {
            // Get the UIView representation of the Slint window
            let slintView = slintWindow.uiView()
            
            // Configure constraints to fill the screen
            slintView.translatesAutoresizingMaskIntoConstraints = false
            window.rootViewController = UIViewController()
            window.rootViewController?.view.addSubview(slintView)
            
            NSLayoutConstraint.activate([
                slintView.topAnchor.constraint(equalTo: window.safeAreaLayoutGuide.topAnchor),
                slintView.leadingAnchor.constraint(equalTo: window.rootViewController!.view.leadingAnchor),
                slintView.trailingAnchor.constraint(equalTo: window.rootViewController!.view.trailingAnchor),
                slintView.bottomAnchor.constraint(equalTo: window.rootViewController!.view.bottomAnchor)
            ])
        }
        
        self.window = window
        window.makeKeyAndVisible()
        
        // 5. Handle URL Contexts (Deep Links) if launched from background/cold start
        if let urlContext = connectionOptions.urlContexts.first {
            self.handleDeepLink(url: urlContext.url)
        }
    }

    func scene(_ scene: UIScene, openURLContexts URLContexts: Set<UIOpenURLContext>) {
        guard let url = URLContexts.first?.url else { return }
        self.handleDeepLink(url: url)
    }

    private func handleDeepLink(url: URL) {
        guard url.scheme == "spotka" else { return }
        
        // Parse the URL and trigger logic in Rust
        // Example: spotka://join?code=XYZ
        if let components = URLComponents(url: url, resolvingAgainstBaseURL: true) {
            if components.host == "join",
               let queryItems = components.queryItems,
               let codeItem = queryItems.first(where: { $0.name == "code" }),
               let code = codeItem.value {
                
                // Call Rust function to handle join request
                // Note: Requires FFI binding exposed in lib.rs
                spotka_core_handle_deep_link(code) 
            }
        }
    }

    func sceneDidDisconnect(_ scene: UIScene) {
        // Called as the scene is being released by the system.
        // Release resources but keep state if possible.
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        // Restart any tasks that were paused (or not yet started) when the scene was inactive.
        // Resume Rust event loop if it was suspended.
    }

    func sceneWillResignActive(_ scene: UIScene) {
        // Called when the scene will move from an active state to an inactive state.
        // Pause Rust timers or P2P activity if necessary to save battery.
    }

    func sceneWillEnterForeground(_ scene: UIScene) {
        // Called as the scene transitions from the background to the foreground.
    }

    func sceneDidEnterBackground(_ scene: UIScene) {
        // Called as the scene transitions from the foreground to the background.
        // Save data, suspend P2P listeners (keep only BLE/Geofence if needed).
    }
}
