/**
 * Decyzja o aktualizacji z Google Play (spec 2026-10-07-wersja-i-automatyczne-aktualizacje).
 * Czysta funkcja — testowana bez telefonu. Kody jak w Play Core / capacitor-app-update:
 * updateAvailability 2 = UPDATE_AVAILABLE, 3 = w trakcie; installStatus 11 = DOWNLOADED.
 */
export interface UpdateInfoLite {
  updateAvailability: number
  installStatus?: number
  flexibleUpdateAllowed?: boolean
  immediateUpdateAllowed?: boolean
}

export type UpdateAction = 'none' | 'flexible' | 'immediate' | 'complete' | 'wait'

const UPDATE_AVAILABLE = 2
const DOWNLOADED = 11

/**
 * - pobrana wcześniej aktualizacja: instaluj, chyba że trwa spacer (wtedy czekaj — restart
 *   apki w trakcie spaceru przerwałby śledzenie trasy);
 * - nowa wersja: pobieranie w tle (flexible), a gdy Play na nie nie pozwala — pełny ekran
 *   aktualizacji (immediate), ale nigdy w trakcie spaceru.
 */
export function decideUpdate(info: UpdateInfoLite, walking: boolean): UpdateAction {
  if (info.installStatus === DOWNLOADED) return walking ? 'wait' : 'complete'
  if (info.updateAvailability !== UPDATE_AVAILABLE) return 'none'
  if (info.flexibleUpdateAllowed) return 'flexible'
  if (info.immediateUpdateAllowed) return walking ? 'wait' : 'immediate'
  return 'none'
}
