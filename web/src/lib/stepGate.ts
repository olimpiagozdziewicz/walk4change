/**
 * SeaSteps — bramka kroków (spec 2026-10-06-czujnik-krokow-auto-pauza).
 *
 * GPS w domu „pływa” o dziesiątki metrów i sam z siebie nie odróżni dryfu od
 * chodzenia (sesja 05.10: 131 m zaliczone przy przesunięciu 1 m). Czujnik
 * kroków to odróżnia — tak jak Strava. Bramka trzyma „pozycję spaceru”, która
 * może zbliżyć się do pozycji GPS najwyżej o tyle metrów, ile pozwalają
 * kroki zrobione od ostatniego przesunięcia. Zero kroków = pozycja stoi.
 * Do serwera idzie ta pozycja, nie surowy GPS — serwer liczy więc metry
 * ≤ kroki × METERS_PER_STEP.
 */

export interface LatLng {
  lat: number
  lng: number
}

/** Hojny krok (średni ~0,75 m) — szybki marsz nie traci dystansu. */
const METERS_PER_STEP = 1.0
/**
 * Sufit zapasu przy krokach przychodzących na bieżąco: chodzenie w kółko po
 * pokoju (GPS stoi, zapas rośnie) nie może potem „zapłacić” za duży skok
 * dryfu. Paczka kroków oddana naraz po wybudzeniu telefonu (zgaszony ekran)
 * jest wyjątkiem — cała jest do wykorzystania, żeby spacer nie tracił metrów.
 */
const MAX_BUDGET_M = 25
/**
 * Brak kroków dłużej niż tyle = auto-pauza. Niewykorzystany zapas kroków
 * wtedy przepada — kroki sprzed zatrzymania nie płacą za dryf, gdy się siedzi.
 */
const MOVING_WINDOW_MS = 10_000

export function metersBetween(a: LatLng, b: LatLng): number {
  const R = 6_371_008.8
  const p1 = (a.lat * Math.PI) / 180
  const p2 = (b.lat * Math.PI) / 180
  const dp = p2 - p1
  const dl = ((b.lng - a.lng) * Math.PI) / 180
  const x = Math.sin(dp / 2) ** 2 + Math.cos(p1) * Math.cos(p2) * Math.sin(dl / 2) ** 2
  return 2 * R * Math.asin(Math.min(1, Math.sqrt(x)))
}

export interface StepGate {
  /** Nowe kroki z czujnika (przyrost, nie suma). */
  onSteps(delta: number, now: number): void
  /** Nowy fix GPS; zwraca pozycję spaceru po bramce. */
  onFix(fix: LatLng, now: number): LatLng
  position(): LatLng | null
  traveledMeters(): number
  /** Czy w ostatnich 10 s były kroki (inaczej: auto-pauza). */
  isMoving(now: number): boolean
}

export function createStepGate(): StepGate {
  let pos: LatLng | null = null
  let budget = 0
  let traveled = 0
  let lastStepAt: number | null = null
  const isMoving = (now: number) => lastStepAt != null && now - lastStepAt < MOVING_WINDOW_MS

  return {
    onSteps(delta, now) {
      if (!(delta > 0)) return
      if (!isMoving(now)) budget = 0
      const add = delta * METERS_PER_STEP
      budget = Math.min(budget + add, Math.max(MAX_BUDGET_M, add))
      lastStepAt = now
    },
    onFix(fix, now) {
      if (!isMoving(now)) budget = 0
      if (!pos) {
        pos = { lat: fix.lat, lng: fix.lng }
        return pos
      }
      const dist = metersBetween(pos, fix)
      const move = Math.min(dist, budget)
      if (move > 0) {
        // Krótkie odcinki — interpolacja liniowa w stopniach wystarcza.
        const f = move / dist
        pos = { lat: pos.lat + (fix.lat - pos.lat) * f, lng: pos.lng + (fix.lng - pos.lng) * f }
        budget -= move
        traveled += move
      }
      return pos
    },
    position: () => pos,
    traveledMeters: () => traveled,
    isMoving,
  }
}
