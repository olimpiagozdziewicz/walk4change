package pl.seasteps.app;

import android.os.Bundle;

import com.getcapacitor.BridgeActivity;

public class MainActivity extends BridgeActivity {
    @Override
    public void onCreate(Bundle savedInstanceState) {
        // plugin lokalny musi być zarejestrowany przed startem mostu
        registerPlugin(WalkSupportPlugin.class);
        super.onCreate(savedInstanceState);
    }
}
