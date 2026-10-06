// Uruchomienie: node --test tests/  (Node 22.6+ czyta TypeScript bez kompilacji)
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createStepGate } from '../src/lib/stepGate.ts'
import { createTraceRecorder, replayTrace } from '../src/lib/walkTrace.ts'

const START = { lat: 54.5189, lng: 18.5305 }
const M = 1 / 111_195
const north = (m: number) => ({ lat: START.lat + m * M, lng: START.lng })
const META = { app: 'test', device: 'node' }

test('odtworzenie zapisu daje ten sam dystans co bramka na żywo', () => {
  const live = createStepGate()
  const rec = createTraceRecorder(META, 0)
  const fix = (m: number, t: number, acc = 8) => { live.onFix(north(m), t); rec.fix(north(m), acc, t) }
  const steps = (d: number, t: number) => { live.onSteps(d, t); rec.steps(d, t) }
  fix(0, 0)
  for (let i = 1; i <= 40; i++) {
    steps(2, i * 1000)
    fix(i * 1.4 + (i % 3) * 2, i * 1000 + 300) // marsz z dryfem
  }
  for (let i = 41; i <= 60; i++) fix(56 + (i % 5) * 9, i * 1000) // stanie, GPS pływa
  const replay = replayTrace(rec.snapshot())
  assert.ok(Math.abs(replay.traveledMeters - live.traveledMeters()) < 0.5, `${replay.traveledMeters} vs ${live.traveledMeters()}`)
  assert.equal(replay.steps, 80)
})

test('zapis nie zawiera bezwzględnych współrzędnych', () => {
  const rec = createTraceRecorder(META, Date.UTC(2026, 9, 7, 18, 30))
  rec.fix(START, 5, Date.UTC(2026, 9, 7, 18, 30, 1))
  rec.fix(north(12), 5, Date.UTC(2026, 9, 7, 18, 30, 9))
  const json = JSON.stringify(rec.snapshot())
  assert.ok(!json.includes('54.5') && !json.includes('18.5'), json)
  assert.ok(json.includes('"day":"2026-10-07"'))
  const f = rec.snapshot().events[1]
  assert.equal(f.k, 'f')
  if (f.k === 'f') assert.ok(Math.abs(f.y - 12) < 0.2 && Math.abs(f.x) < 0.2)
})

test('odtworzenie mierzy opóźnienie pierwszego kroku i pierwszego metra', () => {
  const rec = createTraceRecorder(META, 0)
  rec.mode('gate', 0)
  rec.fix(START, 6, 0)
  // czujnik „połyka” początek: pierwsze kroki dopiero po 9 s
  rec.fix(north(3), 6, 4000)
  rec.steps(12, 9000)
  rec.fix(north(9), 6, 9500)
  const r = replayTrace(rec.snapshot())
  assert.equal(r.firstStepMs, 9000)
  assert.equal(r.firstMoveMs, 9500)
})

test('fix o słabej dokładności jest pomijany jak w apce', () => {
  const rec = createTraceRecorder(META, 0)
  rec.fix(START, 5, 0)
  rec.steps(50, 1000)
  rec.fix(north(40), 80, 1500) // zły fix — apka go odrzuca
  assert.equal(replayTrace(rec.snapshot()).traveledMeters, 0)
})

test('rekorder trzyma najwyżej 6000 zdarzeń', () => {
  const rec = createTraceRecorder(META, 0)
  for (let i = 0; i < 6100; i++) rec.steps(1, i)
  assert.equal(rec.size(), 6000)
})
