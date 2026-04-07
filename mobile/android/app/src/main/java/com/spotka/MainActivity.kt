// mobile/android/app/src/main/java/com/spotka/MainActivity.kt
package com.spotka

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.app.Activity
import android.util.Log
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import slint.android.SlintAndroidView // hypothetical Slint Android backend import

class MainActivity : Activity() {

    companion object {
        private const val TAG = "SpotkaAndroid"
        private const val PERMISSIONS_REQUEST_CODE = 101
        private val REQUIRED_PERMISSIONS = arrayOf(
            Manifest.permission.CAMERA,      // For QR/NFC PING
            Manifest.permission.BLUETOOTH_SCAN, // For BLE discovery (Android 12+)
            Manifest.permission.BLUETOOTH_CONNECT,
            Manifest.permission.ACCESS_FINE_LOCATION, // For Geofencing/P2P
            Manifest.permission.POST_NOTIFICATIONS // For Android 13+
        )
    }

    // Native methods (JNI bindings to Rust)
    external fun spotkaInit(authToken: String): Boolean
    external fun spotkaHandleEvent(eventId: String, data: String)
    external fun spotkaOnPause()
    external fun spotkaOnResume()
    external fun spotkaDestroy()

    // Slint View (UI Container)
    private var slintView: SlintAndroidView? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Log.i(TAG, "MSG_ANDROID_ON_CREATE")

        // 1. Request Permissions
        checkPermissions()

        // 2. Initialize Rust Core
        // In production, authToken comes from encrypted storage or biometric auth
        val authToken = "device_secret_placeholder" 
        val initSuccess = spotkaInit(authToken)
        
        if (!initSuccess) {
            Log.e(TAG, "ERR_RUST_INIT_FAILED")
            // Show error dialog to user
            finish()
            return
        }

        // 3. Setup Slint UI
        // Note: Actual Slint Android integration might vary based on library version
        try {
            slintView = SlintAndroidView(this)
            setContentView(slintView)
            
            // Register callback for Rust to call back into Kotlin (e.g., for Camera/QR)
            registerNativeCallbacks()
            
            Log.i(TAG, "MSG_UI_INITIALIZED")
        } catch (e: Exception) {
            Log.e(TAG, "ERR_UI_INIT_FAILED", e)
        }
    }

    override fun onResume() {
        super.onResume()
        Log.i(TAG, "MSG_ANDROID_ON_RESUME")
        spotkaOnResume() // Notify Rust to wake up P2P/Sync
        slintView?.onResume()
    }

    override fun onPause() {
        super.onPause()
        Log.i(TAG, "MSG_ANDROID_ON_PAUSE")
        spotkaOnPause() // Notify Rust to sleep P2P/Save state
        slintView?.onPause()
    }

    override fun onDestroy() {
        super.onDestroy()
        Log.i(TAG, "MSG_ANDROID_ON_DESTROY")
        spotkaDestroy() // Cleanup Rust resources
        slintView?.onDestroy()
    }

    // Permission Handling
    private fun checkPermissions() {
        val missingPermissions = REQUIRED_PERMISSIONS.filter {
            ContextCompat.checkSelfPermission(this, it) != PackageManager.PERMISSION_GRANTED
        }

        if (missingPermissions.isNotEmpty()) {
            ActivityCompat.requestPermissions(this, missingPermissions.toTypedArray(), PERMISSIONS_REQUEST_CODE)
        }
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == PERMISSIONS_REQUEST_CODE) {
            val allGranted = grantResults.all { it == PackageManager.PERMISSION_GRANTED }
            if (!allGranted) {
                Log.w(TAG, "WARN_PERMISSIONS_DENIED")
                // Handle denial (disable features like PING or Maps)
            } else {
                Log.i(TAG, "MSG_PERMISSIONS_GRANTED")
            }
        }
    }

    // Callbacks registered for Rust to invoke
    private fun registerNativeCallbacks() {
        // Example: Rust calls this when it needs to open Camera for QR
        // This would be implemented via a Java interface passed to Rust or a global registry
        Log.d(TAG, "DBG_CALLBACKS_REGISTERED")
    }
}
