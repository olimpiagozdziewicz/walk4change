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
    /**
     * Dwa czujniki naraz: STEP_DETECTOR daje każdy krok od razu (UI reaguje
     * bez zwłoki), STEP_COUNTER jest dokładny, ale Android może go opóźniać
     * do ~10 s (zmierzone 06.10: kroki i pauza „włączały się po czasie”).
     * Suma = ostatni odczyt licznika + kroki z detektora od tego odczytu;
     * nigdy nie maleje (nadmiar detektora licznik po prostu dogania).
     */
    private float counterBaseline = -1f;
    private long counterSteps = 0;
    private long detectorSinceCounter = 0;
    private long emittedTotal = 0;

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
        Sensor detector = sm == null ? null : sm.getDefaultSensor(Sensor.TYPE_STEP_DETECTOR);
        if (sensor == null) {
            ret.put("available", false);
            ret.put("reason", "sensor");
            call.resolve(ret);
            return;
        }
        stopSensor();
        sensorManager = sm;
        counterBaseline = -1f;
        counterSteps = 0;
        detectorSinceCounter = 0;
        emittedTotal = 0;
        // maxReportLatency 0 = bez paczkowania, kroki przychodzą na bieżąco
        sm.registerListener(this, sensor, SensorManager.SENSOR_DELAY_UI, 0);
        if (detector != null) sm.registerListener(this, detector, SensorManager.SENSOR_DELAY_UI, 0);
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
        int type = event.sensor.getType();
        if (type == Sensor.TYPE_STEP_DETECTOR) {
            detectorSinceCounter++;
        } else if (type == Sensor.TYPE_STEP_COUNTER) {
            float value = event.values[0];
            // Pierwszy odczyt (albo restart telefonu): kroki z detektora sprzed
            // niego już są w sumie — licznik startuje od nich, nie od zera.
            if (counterBaseline < 0f || value < counterBaseline + counterSteps) {
                counterBaseline = value - emittedTotal;
            }
            counterSteps = (long) (value - counterBaseline);
            detectorSinceCounter = 0;
        } else {
            return;
        }
        long total = Math.max(emittedTotal, counterSteps + detectorSinceCounter);
        if (total == emittedTotal && emittedTotal > 0) return;
        emittedTotal = total;
        JSObject data = new JSObject();
        data.put("total", total);
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
