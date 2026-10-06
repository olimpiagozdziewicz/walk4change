// Uruchomienie: node --test tests/  (Node 22.6+ czyta TypeScript bez kompilacji)
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { formatEventWhen, formatEventPlace } from '../src/lib/eventDates.ts'

test('jednodniowe wydarzenie z godzinami', () => {
  assert.equal(formatEventWhen('2026-10-06T09:00:00', '2026-10-06T13:00:00', false), 'wt 6 paź • 09:00–13:00')
})

test('wielodniowe wydarzenie pokazuje zakres dni', () => {
  assert.equal(formatEventWhen('2026-10-06T10:00:00', '2026-10-08T16:00:00', false), 'wt 6 paź, 10:00 – cz 8 paź')
  assert.equal(formatEventWhen('2026-10-06T00:00:00', '2026-10-07T23:59:59', true), 'wt 6 paź – śr 7 paź')
})

test('całodniowe i bez końca', () => {
  assert.equal(formatEventWhen('2026-10-11T00:00:00', '2026-10-11T23:59:59', true), 'nd 11 paź • cały dzień')
  assert.equal(formatEventWhen('2026-10-11T18:30:00', null, false), 'nd 11 paź • 18:30')
})

test('nieznany format zwraca surowy tekst', () => {
  assert.equal(formatEventWhen('jutro', null, false), 'jutro')
})

test('miejsce bez powtórek i null dla online', () => {
  assert.equal(formatEventPlace('Kraków', 'Kraków'), 'Kraków')
  assert.equal(formatEventPlace('Centrum Nauki', 'Gdańsk'), 'Centrum Nauki · Gdańsk')
  assert.equal(formatEventPlace(null, null), null)
  assert.equal(formatEventPlace('  ', null), null)
})
