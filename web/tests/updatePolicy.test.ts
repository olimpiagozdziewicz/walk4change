// Uruchomienie: node --test tests/  (Node 22.6+ czyta TypeScript bez kompilacji)
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { decideUpdate } from '../src/lib/updatePolicy.ts'

test('brak nowej wersji = nic', () => {
  assert.equal(decideUpdate({ updateAvailability: 1 }, false), 'none')
})

test('nowa wersja: pobieranie w tle, także w trakcie spaceru', () => {
  assert.equal(decideUpdate({ updateAvailability: 2, flexibleUpdateAllowed: true, immediateUpdateAllowed: true }, false), 'flexible')
  assert.equal(decideUpdate({ updateAvailability: 2, flexibleUpdateAllowed: true }, true), 'flexible')
})

test('pobrana aktualizacja instaluje się dopiero po spacerze', () => {
  assert.equal(decideUpdate({ updateAvailability: 2, installStatus: 11 }, false), 'complete')
  assert.equal(decideUpdate({ updateAvailability: 2, installStatus: 11 }, true), 'wait')
})

test('tylko pełnoekranowa aktualizacja — nigdy w trakcie spaceru', () => {
  assert.equal(decideUpdate({ updateAvailability: 2, immediateUpdateAllowed: true }, false), 'immediate')
  assert.equal(decideUpdate({ updateAvailability: 2, immediateUpdateAllowed: true }, true), 'wait')
})

test('Play nie pozwala na żaden tryb = nic', () => {
  assert.equal(decideUpdate({ updateAvailability: 2 }, false), 'none')
})
