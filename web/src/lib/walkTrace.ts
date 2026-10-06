/**
 * SeaSteps — zapis spaceru do zgłoszenia błędu
 * (spec 2026-10-07-zapis-spaceru-do-zgloszenia-bledu).
 *
 * Słowne zgłoszenie typu „pauza muli” jest nie do sprawdzenia. Rekorder
 * zapisuje surowe zdarzenia spaceru (kroki, fixy GPS, tryb), a `replayTrace`
 * puszcza je przez tę samą bramkę kroków co apka. Ten sam zapis po poprawce
 * pokazuje, czy błąd zniknął.
 *
 * Prywatność: fix to przesunięcie w metrach od pierwszego fixu (x = wschód,
 * y = północ), bez bezwzględnych współrzędnych. Z pliku nie wynika, gdzie
 * ktoś był.
 */
import { createStepGate, type LatLng } from './stepGate.ts'

/** Ten sam próg co w Walk.tsx: fix o gorszej dokładności nie trafia do bramki. */
export const MAX_FIX_ACCURACY_M = 35
const MAX_EVENTS = 6000
const M_PER_DEG_LAT = 111_195

export type TraceEvent =
  | { t: number; k: 's'; d: number }
  | { t: number; k: 'f'; x: number; y: number; a: number | null }
  | { t: number; k: 'm'; m: string }

export interface WalkTrace {
  v: 1
  app: string
  device: string
  /** Data startu (dzień, bez godziny i miejsca) — do kojarzenia zgłoszeń. */
  day: string
  events: TraceEvent[]
}

export interface TraceRecorder {
  steps(delta: number, now: number): void
  fix(fix: LatLng, accuracy: number | null, now: number): void
  mode(mode: string, now: number): void
  size(): number
  snapshot(): WalkTrace
}

const round1 = (n: number) => Math.round(n * 10) / 10

export function createTraceRecorder(meta: { app: string; device: string }, startedAt: number): TraceRecorder {
  const events: TraceEvent[] = []
  let origin: LatLng | null = null
  const push = (e: TraceEvent) => {
    events.push(e)
    if (events.length > MAX_EVENTS) events.splice(0, events.length - MAX_EVENTS)
  }
  const rel = (now: number) => Math.max(0, now - startedAt)

  return {
    steps(delta, now) {
      if (delta > 0) push({ t: rel(now), k: 's', d: delta })
    },
    fix(fix, accuracy, now) {
      if (!origin) origin = { lat: fix.lat, lng: fix.lng }
      const y = (fix.lat - origin.lat) * M_PER_DEG_LAT
      const x = (fix.lng - origin.lng) * M_PER_DEG_LAT * Math.cos((origin.lat * Math.PI) / 180)
      push({ t: rel(now), k: 'f', x: round1(x), y: round1(y), a: accuracy == null ? null : round1(accuracy) })
    },
    mode(mode, now) {
      push({ t: rel(now), k: 'm', m: mode })
    },
    size: () => events.length,
    snapshot: () => ({
      v: 1,
      app: meta.app,
      device: meta.device,
      day: new Date(startedAt).toISOString().slice(0, 10),
      events: events.slice(),
    }),
  }
}

export interface ReplayPoint {
  t: number
  traveled: number
  moving: boolean
}

export interface ReplayResult {
  traveledMeters: number
  steps: number
  /** ms od startu do pierwszego zdarzenia kroków z czujnika (null = brak kroków). */
  firstStepMs: number | null
  /** ms od startu do chwili, gdy bramka pierwszy raz przesunęła pozycję. */
  firstMoveMs: number | null
  /** Oś czasu po każdym zdarzeniu — do szukania „zawieszek” pauzy. */
  timeline: ReplayPoint[]
}

/** Punkt odniesienia odtwarzania — dowolny; bramka liczy tylko odległości. */
const REPLAY_ORIGIN: LatLng = { lat: 54.5, lng: 18.5 }

function toLatLng(x: number, y: number): LatLng {
  return {
    lat: REPLAY_ORIGIN.lat + y / M_PER_DEG_LAT,
    lng: REPLAY_ORIGIN.lng + x / (M_PER_DEG_LAT * Math.cos((REPLAY_ORIGIN.lat * Math.PI) / 180)),
  }
}

/** Puszcza zapis przez bramkę kroków tak jak Walk.tsx (tryb 'gate'). */
export function replayTrace(trace: WalkTrace): ReplayResult {
  const gate = createStepGate()
  let steps = 0
  let firstStepMs: number | null = null
  let firstMoveMs: number | null = null
  const timeline: ReplayPoint[] = []
  for (const e of trace.events) {
    if (e.k === 's') {
      gate.onSteps(e.d, e.t)
      steps += e.d
      if (firstStepMs == null) firstStepMs = e.t
    } else if (e.k === 'f') {
      if (e.a != null && e.a > MAX_FIX_ACCURACY_M) continue
      gate.onFix(toLatLng(e.x, e.y), e.t)
      if (firstMoveMs == null && gate.traveledMeters() > 0) firstMoveMs = e.t
    } else {
      continue
    }
    timeline.push({ t: e.t, traveled: round1(gate.traveledMeters()), moving: gate.isMoving(e.t) })
  }
  return { traveledMeters: gate.traveledMeters(), steps, firstStepMs, firstMoveMs, timeline }
}
