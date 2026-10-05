package pl.seasteps.app;

import android.Manifest;

import com.getcapacitor.Plugin;
import com.getcapacitor.annotation.CapacitorPlugin;
import com.getcapacitor.annotation.Permission;

/**
 * Zgoda na powiadomienia (Android 13+) przed spacerem — bez niej powiadomienie
 * usługi GPS „Spacer trwa” jest niewidoczne (spec 2026-10-05). Metody
 * checkPermissions/requestPermissions daje bazowa klasa Plugin.
 */
@CapacitorPlugin(
        name = "WalkSupport",
        permissions = {
                @Permission(strings = { Manifest.permission.POST_NOTIFICATIONS }, alias = "notifications")
        }
)
public class WalkSupportPlugin extends Plugin {}
