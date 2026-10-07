/**
 * Numer wersji i automatyczne aktualizacje z Google Play
 * (spec 2026-10-07-wersja-i-automatyczne-aktualizacje).
 *
 * Apka sprawdza Google Play przy starcie i po każdym powrocie na ekran. Nową
 * wersję pobiera w tle, a instaluje (krótki restart) dopiero, gdy nie trwa
 * spacer. Działa tylko w apce z Google Play — w przeglądarce i w APK
 * wgranym ręcznie nic się nie dzieje.
 */
import { Capacitor } from '@capacitor/core'
import { decideUpdate } from './updatePolicy'
import { isWalkActive } from './walkGuard'

/** „1.0.18 (18)” w apce, „wersja przeglądarkowa” w przeglądarce. */
export async function appVersionLabel(): Promise<string> {
  if (!Capacitor.isNativePlatform()) return 'wersja przeglądarkowa'
  try {
    const { App } = await import('@capacitor/app')
    const info = await App.getInfo()
    return `${info.version} (${info.build})`
  } catch {
    return 'nieznana'
  }
}

let started = false
let busy = false
let waitTimer: number | null = null

async function checkOnce(): Promise<void> {
  if (busy) return
  busy = true
  try {
    const { AppUpdate } = await import('@capawesome/capacitor-app-update')
    const info = await AppUpdate.getAppUpdateInfo()
    const action = decideUpdate(info, isWalkActive())
    if (action === 'flexible') await AppUpdate.startFlexibleUpdate()
    else if (action === 'immediate') await AppUpdate.performImmediateUpdate()
    else if (action === 'complete') await AppUpdate.completeFlexibleUpdate()
    else if (action === 'wait') scheduleRetry()
  } catch {
    // Brak Google Play / apka spoza sklepu / brak sieci — spróbujemy przy kolejnym powrocie.
  } finally {
    busy = false
  }
}

/** Po spacerze: co minutę sprawdzamy, czy można już zainstalować pobraną wersję. */
function scheduleRetry() {
  if (waitTimer != null) return
  waitTimer = window.setTimeout(() => {
    waitTimer = null
    void checkOnce()
  }, 60_000)
}

export function startAutoUpdates(): void {
  if (started || Capacitor.getPlatform() !== 'android') return
  started = true
  void (async () => {
    try {
      const [{ App }, { AppUpdate }] = await Promise.all([
        import('@capacitor/app'),
        import('@capawesome/capacitor-app-update'),
      ])
      // Pobieranie w tle skończone → instalacja, gdy nie trwa spacer.
      await AppUpdate.addListener('onFlexibleUpdateStateChange', (state) => {
        if (state.installStatus === 11) void checkOnce()
      })
      await App.addListener('resume', () => void checkOnce())
    } catch {
      return
    }
    void checkOnce()
  })()
}
