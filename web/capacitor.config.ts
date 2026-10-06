import type { CapacitorConfig } from '@capacitor/cli'

// Natywna apka Android (spec 2026-07-13). Bundlujemy build webowy (webDir) —
// NIE server.url: zdalny URL kłóci się z pluginami i polityką Google Play.
// PWA na seasteps.pl zostaje bez zmian (ścieżka dla iPhone'ów).
const config: CapacitorConfig = {
  appId: 'pl.seasteps.app',
  appName: 'SeaSteps',
  webDir: 'dist',
  plugins: {
    // jasna apka: ciemne ikonki paska stanu i nawigacji niezaleznie od trybu ciemnego telefonu
    // (DEFAULT bral styl z systemu i dawal biale ikonki na jasnym tle - niewidoczne)
    SystemBars: { style: 'LIGHT' },
  },
}

export default config
