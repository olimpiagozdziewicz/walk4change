export interface SavedWalk {
  id: string
  dateLabel: string
  durationSec: number
  steps: number
  points: number
  withSomeone: boolean
  inNature: boolean
  withDog?: boolean
  place: string
  routeSeed: number
  /** emoji (placeholder) albo data:/http URL zdjęcia */
  photos: string[]
}

const KEY = 'ss-walks'

export function getWalks(): SavedWalk[] {
  try {
    const v = localStorage.getItem(KEY)
    return v ? (JSON.parse(v) as SavedWalk[]) : []
  } catch {
    return []
  }
}

export function addWalk(w: SavedWalk): SavedWalk[] {
  const existing = getWalks()
  // Dedupe po sessionId — reentrancy guard w Walk.tsx powinien to wykluczyć,
  // ale to tania, dodatkowa siatka przeciw duplikatom w historii.
  if (existing.some((x) => x.id === w.id)) return existing
  const list = [w, ...existing]
  try {
    localStorage.setItem(KEY, JSON.stringify(list))
  } catch {
    /* quota — ignorujemy w demo */
  }
  return list
}
