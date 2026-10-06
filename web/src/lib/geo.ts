/**
 * SeaSteps — abstrakcja nad GPS (spec 2026-07-13, cz. Android).
 *
 * Przeglądarka/PWA: navigator.geolocation.watchPosition (jak dotąd — ścieżka
 * dla iPhone'ów zostaje bez zmian). Apka natywna (Capacitor Android):
 * @capacitor-community/background-geolocation — śledzenie działa też przy
 * zgaszonym ekranie (foreground service z widoczną notyfikacją), czyli główny
 * brak PWA na spacerze. Logika punktów zostaje w Walk.tsx — tu tylko źródło
 * pozycji.
 */

import { Capacitor, registerPlugin } from '@capacitor/core'

export interface GeoFix {
  lat: number
  lng: number
  /** Promień niepewności w metrach (null gdy nieznany). */
  accuracy: number | null
}

export interface GeoWatch {
  stop: () => void
}

/** Czy działamy w natywnej apce (Capacitor), a nie w przeglądarce/PWA. */
export function isNativeApp(): boolean {
  return Capacitor.isNativePlatform()
}

// Typy pluginu (definitions.d.ts @capacitor-community/background-geolocation).
interface BgLocation {
  latitude: number
  longitude: number
  accuracy: number
  simulated: boolean
  time: number | null
}
interface BgError extends Error {
  code?: string
}
interface BgPlugin {
  addWatcher(
    options: {
      backgroundMessage?: string
      backgroundTitle?: string
      requestPermissions?: boolean
      stale?: boolean
      distanceFilter?: number
    },
    callback: (position?: BgLocation, error?: BgError) => void,
  ): Promise<string>
  removeWatcher(options: { id: string }): Promise<void>
  openSettings(): Promise<void>
  // z bazowej klasy Plugin (Capacitor) — alias "location" z adnotacji pluginu
  checkPermissions(): Promise<{ location: string }>
  requestPermissions(): Promise<{ location: string }>
}

const BackgroundGeolocation = registerPlugin<BgPlugin>('BackgroundGeolocation')

// Nasz mini-plugin z MainActivity: zgoda na powiadomienia (Android 13+, bez
// niej powiadomienie „Spacer trwa” jest niewidoczne) i na aktywność fizyczną
// oraz czujnik kroków (spec 2026-10-06-czujnik-krokow-auto-pauza).
interface WalkSupportPlugin {
  requestPermissions(): Promise<{ notifications: string; activity: string }>
  startStepCounter(): Promise<{ available: boolean; reason?: 'permission' | 'sensor' }>
  stopStepCounter(): Promise<void>
  addListener(event: 'steps', cb: (e: { total: number }) => void): Promise<{ remove: () => Promise<void> }>
}
const WalkSupport = registerPlugin<WalkSupportPlugin>('WalkSupport')

// Zgody pytamy raz, przy starcie GPS — czujnik kroków czeka na ten sam moment,
// żeby nie startować przed odpowiedzią na systemowe okienko.
let permissionsReady: Promise<boolean> | null = null

/**
 * Zgoda na lokalizację PRZED addWatcher. Plugin przy braku zgody prosi o nią,
 * ale nie czeka i od razu woła startForeground(type=location) — Android 14+
 * odrzuca to bez zgody, a po jej nadaniu plugin już nie ponawia promocji
 * usługi. Bez usługi pierwszoplanowej GPS w tle zamiera (bug 05.10.2026).
 */
async function ensureNativePermissions(): Promise<boolean> {
  try {
    await WalkSupport.requestPermissions()
  } catch {
    /* stary build bez pluginu albo Android < 13 — spacer działa dalej */
  }
  let state = (await BackgroundGeolocation.checkPermissions()).location
  if (state !== 'granted') state = (await BackgroundGeolocation.requestPermissions()).location
  return state === 'granted'
}

/**
 * Nasłuch pozycji. Zwraca uchwyt ze `stop()` — bezpieczny do wywołania
 * w każdym momencie (w natywnym trybie czeka aż watcher się zarejestruje).
 */
export function watchPosition(
  onFix: (fix: GeoFix) => void,
  onError: (message: string) => void,
): GeoWatch {
  if (isNativeApp()) {
    let stopped = false
    permissionsReady = ensureNativePermissions()
    const idPromise = permissionsReady.then((granted) => {
      if (!granted) throw Object.assign(new Error('Permission denied.'), { code: 'NOT_AUTHORIZED' })
      if (stopped) throw new Error('stopped')
      return BackgroundGeolocation.addWatcher(
      {
        backgroundTitle: 'Spacer trwa',
        backgroundMessage: 'SeaSteps liczy Twoją trasę i punkty.',
        // zgoda już jest (ensureNativePermissions) — patrz komentarz wyżej
        requestPermissions: false,
        stale: false,
        // Serwer i tak ma deadband 5 m; filtr 3 m tnie szum bez utraty kroków.
        distanceFilter: 3,
      },
      (position, error) => {
        if (error) {
          onError(
            error.code === 'NOT_AUTHORIZED'
              ? 'Brak zgody na lokalizację. Nadaj uprawnienie w ustawieniach aplikacji.'
              : `GPS niedostępny: ${error.message}`,
          )
          return
        }
        if (!position) return
        // Pozycje symulowane (fake-GPS) odrzucamy — anty-fraud po stronie klienta.
        if (position.simulated) return
        onFix({ lat: position.latitude, lng: position.longitude, accuracy: position.accuracy ?? null })
      },
      )
    })
    idPromise.catch((e: unknown) => {
      if (stopped) return
      onError(
        (e as BgError)?.code === 'NOT_AUTHORIZED'
          ? 'Brak zgody na lokalizację. Nadaj uprawnienie w ustawieniach aplikacji.'
          : `Nie udało się uruchomić GPS: ${e instanceof Error ? e.message : 'nieznany błąd'}`,
      )
    })
    return {
      stop: () => {
        stopped = true
        idPromise.then((id) => BackgroundGeolocation.removeWatcher({ id })).catch(() => {})
      },
    }
  }

  if (!('geolocation' in navigator)) {
    onError('Brak GPS w tej przeglądarce.')
    return { stop: () => {} }
  }

  const watchId = navigator.geolocation.watchPosition(
    (pos) =>
      onFix({
        lat: pos.coords.latitude,
        lng: pos.coords.longitude,
        accuracy: typeof pos.coords.accuracy === 'number' ? pos.coords.accuracy : null,
      }),
    (err) => onError(`GPS niedostępny: ${err.message}. Włącz lokalizację i odśwież.`),
    { enableHighAccuracy: true, maximumAge: 1000, timeout: 15000 },
  )
  return { stop: () => navigator.geolocation.clearWatch(watchId) }
}

export interface StepWatch {
  stop: () => void
}

/**
 * Czujnik kroków telefonu (tylko apka natywna). `onTotal` dostaje sumę kroków
 * od startu. Zwraca null, gdy czujnika nie ma, brak zgody albo to przeglądarka —
 * spacer liczy się wtedy jak dotąd, z samego GPS.
 */
export async function watchSteps(onTotal: (total: number) => void): Promise<StepWatch | null> {
  if (!isNativeApp()) return null
  try {
    // Zwykle zgody już pyta watchPosition; gdyby nie — pytamy sami.
    await (permissionsReady ??= ensureNativePermissions())
    const res = await WalkSupport.startStepCounter()
    if (!res.available) return null
    const listener = await WalkSupport.addListener('steps', (e) => onTotal(e.total))
    return {
      stop: () => {
        void listener.remove().catch(() => {})
        void WalkSupport.stopStepCounter().catch(() => {})
      },
    }
  } catch {
    return null // stary build apki bez czujnika w pluginie
  }
}

const DISCLOSURE_KEY = 'ss-geo-disclosure'

/**
 * Prominent disclosure (wymóg Google Play przy lokalizacji w tle):
 * dialog w apce PRZED pierwszą prośbą o uprawnienie. Web/PWA nie wymaga.
 */
export function needsLocationDisclosure(): boolean {
  if (!isNativeApp()) return false
  try {
    return localStorage.getItem(DISCLOSURE_KEY) !== '1'
  } catch {
    return false
  }
}

export function markLocationDisclosureAccepted(): void {
  try {
    localStorage.setItem(DISCLOSURE_KEY, '1')
  } catch {
    /* ignore */
  }
}
