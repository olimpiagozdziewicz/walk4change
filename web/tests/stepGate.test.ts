// Uruchomienie: node --test tests/  (Node 22.6+ czyta TypeScript bez kompilacji)
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createStepGate, metersBetween } from '../src/lib/stepGate.ts'

const START = { lat: 54.5189, lng: 18.5305 }
// ~1 m na północ w stopniach szerokości
const M = 1 / 111_195

const north = (m: number) => ({ lat: START.lat + m * M, lng: START.lng })

test('bez kroków pozycja stoi mimo dryfu GPS', () => {
  const g = createStepGate()
  g.onFix(START, 0)
  for (const [i, m] of [10, 74, 98, 85, 47, 33, 28, 9, 2, 1].entries()) {
    g.onFix(north(m), (i + 1) * 5000)
  }
  assert.equal(metersBetween(g.position()!, START), 0)
  assert.equal(g.traveledMeters(), 0)
})

test('idąc, pozycja nadąża za GPS', () => {
  const g = createStepGate()
  g.onFix(START, 0)
  for (let s = 1; s <= 60; s++) {
    g.onSteps(2, s * 1000) // ~2 kroki/s
    g.onFix(north(1.4 * s), s * 1000) // 1,4 m/s
  }
  assert.ok(metersBetween(g.position()!, north(84)) < 1)
  assert.ok(Math.abs(g.traveledMeters() - 84) < 1)
})

test('dystans nigdy nie przekracza kroków × 1 m', () => {
  const g = createStepGate()
  g.onFix(START, 0)
  g.onSteps(10, 1000)
  g.onFix(north(98), 2000) // skok dryfu przy 10 krokach
  assert.ok(Math.abs(g.traveledMeters() - 10) < 0.01)
})

test('zapas kroków wygasa przy auto-pauzie (chodzenie po pokoju, potem siedzenie)', () => {
  const g = createStepGate()
  g.onFix(START, 0)
  g.onSteps(200, 1000) // 200 kroków w kółko po pokoju, GPS stoi
  g.onFix(START, 2000)
  g.onFix(north(98), 60_000) // siedzi, GPS odpływa
  assert.equal(g.traveledMeters(), 0)
})

test('kroki na bieżąco (chodzenie w kółko po pokoju) nie zbierają zapasu ponad 25 m', () => {
  const g = createStepGate()
  g.onFix(START, 0)
  for (let s = 1; s <= 120; s++) {
    g.onSteps(2, s * 1000) // 2 min chodzenia w kółko, GPS stoi
    g.onFix(START, s * 1000)
  }
  g.onSteps(2, 121_000)
  g.onFix(north(98), 121_000) // skok dryfu w trakcie chodzenia
  assert.ok(g.traveledMeters() <= 25.01, `zaliczono ${g.traveledMeters()} m`)
})

test('paczka kroków dostarczona naraz (ekran zgaszony) jest w pełni do wykorzystania', () => {
  const g = createStepGate()
  g.onFix(START, 0)
  g.onSteps(150, 90_000) // czujnik oddał 150 kroków po wybudzeniu
  g.onFix(north(110), 90_500)
  assert.ok(Math.abs(g.traveledMeters() - 110) < 0.01)
})

test('auto-pauza: ruch tylko gdy były kroki w ostatnich 6 s', () => {
  const g = createStepGate()
  assert.equal(g.isMoving(0), false)
  g.onSteps(3, 1000)
  assert.equal(g.isMoving(5000), true)
  assert.equal(g.isMoving(8000), false)
})

test('pakiet zero kroków nie włącza ruchu ani dystansu', () => {
  const g = createStepGate()
  g.onFix(START, 0)
  g.onSteps(0, 500)
  g.onFix(north(30), 1000)
  assert.equal(g.isMoving(1000), false)
  assert.equal(g.traveledMeters(), 0)
})
