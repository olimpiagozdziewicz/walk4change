package pl.seasteps.app;

import android.Manifest;
import android.content.Context;
import android.content.pm.PackageManager;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.os.Build;

import androidx.core.content.ContextCompat;

import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import com.getcapacitor.annotation.Permission;

/**
 * Wsparcie spaceru:
 * - zgoda na powiadomienia (Android 13+) przed spacerem — bez niej powiadomienie
 *   usługi GPS „Spacer trwa” jest niewidoczne (spec 2026-10-05);
 * - czujnik kroków telefonu (spec 2026-10-06-czujnik-krokow-auto-pauza) — apka
 *   wie, czy idziesz; bez kroków dryf GPS nie nalicza metrów.
 * Metody checkPermissions/requestPermissions daje bazowa klasa Plugin.
 */
@CapacitorPlugin(
        name = "WalkSupport",
        permissions = {
                @Permission(strings = { Manifest.permission.POST_NOTIFICATIONS }, alias = "notifications"),
                @Permission(strings = { Manifest.permission.ACTIVITY_RECOGNITION }, alias = "activity")
        }
)
public class WalkSupportPlugin extends Plugin implements SensorEventListener {

    private SensorManager sensorManager;
    /** Wartość TYPE_STEP_COUNTER (suma od uruchomienia telefonu) z pierwszego odczytu. */
    private float baseline = -1f;

    /**
     * Start liczenia kroków. Zwraca {available:false, reason} gdy nie ma czujnika
     * albo zgody — apka wraca wtedy do liczenia z samego GPS. Kroki idą
     * zdarzeniem "steps" jako {total} od startu (suma, nie przyrost — zgubione
     * zdarzenie przy zgaszonym ekranie niczego nie psuje).
     */
    @PluginMethod
    public void startStepCounter(PluginCall call) {
        JSObject ret = new JSObject();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q
                && ContextCompat.checkSelfPermission(getContext(), Manifest.permission.ACTIVITY_RECOGNITION)
                        != PackageManager.PERMISSION_GRANTED) {
            ret.put("available", false);
            ret.put("reason", "permission");
            call.resolve(ret);
            return;
        }
        SensorManager sm = (SensorManager) getContext().getSystemService(Context.SENSOR_SERVICE);
        Sensor sensor = sm == null ? null : sm.getDefaultSensor(Sensor.TYPE_STEP_COUNTER);
        if (sensor == null) {
            ret.put("available", false);
            ret.put("reason", "sensor");
            call.resolve(ret);
            return;
        }
        stopSensor();
        sensorManager = sm;
        baseline = -1f;
        // maxReportLatency 0 = bez paczkowania, kroki przychodzą na bieżąco
        sm.registerListener(this, sensor, SensorManager.SENSOR_DELAY_UI, 0);
        ret.put("available", true);
        call.resolve(ret);
    }

    @PluginMethod
    public void stopStepCounter(PluginCall call) {
        stopSensor();
        call.resolve();
    }

    @Override
    public void onSensorChanged(SensorEvent event) {
        if (event.sensor.getType() != Sensor.TYPE_STEP_COUNTER) return;
        float value = event.values[0];
        if (baseline < 0f || value < baseline) baseline = value; // pierwszy odczyt / restart telefonu
        JSObject data = new JSObject();
        data.put("total", (long) (value - baseline));
        notifyListeners("steps", data);
    }

    @Override
    public void onAccuracyChanged(Sensor sensor, int accuracy) {}

    @Override
    protected void handleOnDestroy() {
        stopSensor();
    }

    private void stopSensor() {
        if (sensorManager != null) {
            sensorManager.unregisterListener(this);
            sensorManager = null;
        }
    }
}
