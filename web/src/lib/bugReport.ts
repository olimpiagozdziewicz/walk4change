/**
 * „Zgłoś błąd” (spec 2026-10-07-zapis-spaceru-do-zgloszenia-bledu).
 * Zgłoszenie idzie prosto do SeaSteps (POST /bug-reports) razem z wersją
 * apki, modelem telefonu i — jeśli użytkownik się zgodzi — zapisem ostatniego
 * spaceru, który da się odtworzyć krok po kroku (`npm run replay`).
 */
import { Capacitor } from '@capacitor/core'
import { apiRequest } from './http'
import type { WalkTrace } from './walkTrace'

export type BugCategory = 'walk' | 'steps' | 'map' | 'login' | 'other'

export const BUG_CATEGORIES: { value: BugCategory; label: string }[] = [
  { value: 'walk', label: 'Spacer: pauza, czas, punkty' },
  { value: 'steps', label: 'Kroki lub dystans' },
  { value: 'map', label: 'Mapa lub lokalizacja' },
  { value: 'login', label: 'Logowanie lub konto' },
  { value: 'other', label: 'Coś innego' },
]

export const DESCRIPTION_MAX = 2000

let lastTrace: WalkTrace | null = null

/** Walk.tsx zostawia tu zapis zakończonego spaceru (tylko w pamięci). */
export function setLastWalkTrace(trace: WalkTrace | null): void {
  lastTrace = trace && trace.events.length > 0 ? trace : null
}

export function getLastWalkTrace(): WalkTrace | null {
  return lastTrace
}

/** Model telefonu z user-agenta WebView, np. „RMX5051” — bez reszty UA. */
export function deviceModel(): string {
  const m = /Android [\d.]+; ([^;)]+)/.exec(navigator.userAgent)
  return m ? m[1].trim().slice(0, 80) : 'nieznany'
}

async function appVersion(): Promise<string | null> {
  if (!Capacitor.isNativePlatform()) return 'web'
  try {
    const { App } = await import('@capacitor/app')
    const info = await App.getInfo()
    return `${info.version} (${info.build})`.slice(0, 40)
  } catch {
    return null
  }
}

export async function sendBugReport(input: {
  category: BugCategory
  description: string
  screen: string
  includeTrace: boolean
}): Promise<void> {
  const version = await appVersion()
  await apiRequest('/bug-reports', {
    method: 'POST',
    body: {
      category: input.category,
      description: input.description.trim().slice(0, DESCRIPTION_MAX),
      app_version: version,
      device: deviceModel(),
      platform: Capacitor.getPlatform(),
      screen: input.screen.slice(0, 60),
      trace: input.includeTrace && lastTrace ? { ...lastTrace, app: version ?? lastTrace.app } : null,
    },
  })
}
