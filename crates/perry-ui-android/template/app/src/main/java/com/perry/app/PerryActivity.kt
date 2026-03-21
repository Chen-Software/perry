package com.perry.app

import android.app.Activity
import android.content.pm.PackageManager
import android.os.Bundle
import android.widget.FrameLayout

/**
 * Minimal Activity that hosts a Perry-compiled native UI.
 *
 * Lifecycle:
 * 1. onCreate: create root FrameLayout, init PerryBridge, spawn native thread
 * 2. Native thread runs the compiled TypeScript (which creates widgets via JNI)
 * 3. Native thread calls App() which blocks forever
 * 4. onDestroy: signal native thread to unpark and exit
 */
class PerryActivity : Activity() {

    private lateinit var rootLayout: FrameLayout
    private var nativeThread: Thread? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Switch from splash theme to normal theme before inflating layout
        setTheme(android.R.style.Theme_Material_Light_NoActionBar)

        rootLayout = FrameLayout(this)
        setContentView(rootLayout)

        // Initialize the bridge with this Activity
        PerryBridge.init(this, rootLayout)

        // Load optional native libraries (e.g. hone-editor) before perry_app
        // so their JNI_OnLoad initializes before symbols are resolved
        try { System.loadLibrary("hone_editor_android") } catch (_: UnsatisfiedLinkError) {}

        // Load the native library (the compiled Perry app)
        System.loadLibrary("perry_app")

        // Initialize JNI cache on the UI thread first
        PerryBridge.nativeInit()

        // Spawn native init thread — this runs the compiled TypeScript main()
        nativeThread = Thread {
            // This calls the entry point of the compiled TypeScript.
            // It will create widgets via JNI, then call App() which blocks.
            PerryBridge.nativeMain()
        }.apply {
            name = "perry-native"
            isDaemon = true
            start()
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == 43) { // LOCATION_PERMISSION_REQUEST
            val granted = grantResults.isNotEmpty() &&
                grantResults[0] == PackageManager.PERMISSION_GRANTED
            PerryBridge.onLocationPermissionResult(granted)
        } else if (requestCode == 44) { // AUDIO_PERMISSION_REQUEST
            val granted = grantResults.isNotEmpty() &&
                grantResults[0] == PackageManager.PERMISSION_GRANTED
            PerryBridge.onAudioPermissionResult(granted)
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        PerryBridge.nativeShutdown()
    }

}
