package com.perry.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Routes notification taps back to the Perry runtime (#97).
 *
 * Registered in `AndroidManifest.xml` under action
 * `com.perry.app.NOTIFICATION_TAP`. `PerryBridge.sendNotification` (and
 * future scheduled-notification posters) attaches a `PendingIntent`
 * targeting this receiver via `setContentIntent`. When the user taps the
 * notification, Android invokes `onReceive` on the main thread and we
 * forward the `id` extra to the native side via
 * `PerryBridge.nativeNotificationTap`, which dispatches to the JS closure
 * that was registered via `notificationOnTap`.
 *
 * `id` is whatever string the notification was posted with — for #94's
 * `notificationSend` it's the fixed `"perry_notification"`; once #96
 * scheduled notifications land on Android, it'll be the user-supplied id.
 */
class PerryNotificationReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val id = intent.getStringExtra("id")
        if (id == null) {
            Log.w("PerryNotification", "tap intent missing id extra")
            return
        }
        try {
            PerryBridge.nativeNotificationTap(id)
        } catch (e: UnsatisfiedLinkError) {
            // Native lib not loaded (process died after the notification
            // was posted, OS re-cold-started us just for the broadcast).
            // Background re-launch is #98, not #97 — log and drop.
            Log.w("PerryNotification", "nativeNotificationTap unavailable; tap dropped", e)
        }
    }
}
