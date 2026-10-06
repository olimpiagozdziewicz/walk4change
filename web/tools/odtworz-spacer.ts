// Odtwarzanie zapisu spaceru ze zgłoszenia (spec 2026-10-07-zapis-spaceru-do-zgloszenia-bledu).
// Użycie: npm run replay -- sciezka/seasteps-zapis-spaceru-RRRR-MM-DD.json
import { readFileSync } from 'node:fs'
import { replayTrace, type WalkTrace } from '../src/lib/walkTrace.ts'

const file = process.argv[2]
if (!file) {
  console.error('Podaj plik zapisu: npm run replay -- plik.json')
  process.exit(1)
}
const trace = JSON.parse(readFileSync(file, 'utf8')) as WalkTrace & { note?: string }
const r = replayTrace(trace)
const s = (ms: number | null) => (ms == null ? 'brak' : `${(ms / 1000).toFixed(1)} s`)

console.log(`apka ${trace.app} | telefon ${trace.device} | dzień ${trace.day} | zdarzeń ${trace.events.length}`)
if (trace.note) console.log(`opis: ${trace.note}`)
console.log(`kroki ${r.steps} | dystans po bramce ${r.traveledMeters.toFixed(1)} m`)
console.log(`pierwszy krok z czujnika po ${s(r.firstStepMs)} | pierwszy metr po ${s(r.firstMoveMs)}`)

// Odcinki auto-pauzy po pierwszym kroku: tu widać „pauza muli”.
let pauseFrom: number | null = null
for (const p of r.timeline) {
  if (r.firstStepMs == null || p.t < r.firstStepMs) continue
  if (!p.moving && pauseFrom == null) pauseFrom = p.t
  if (p.moving && pauseFrom != null) {
    console.log(`pauza ${s(pauseFrom)} → ${s(p.t)} (${s(p.t - pauseFrom)})`)
    pauseFrom = null
  }
}
