export interface SavedWalk {
  id: string
  dateLabel: string
  durationSec: number
  steps: number
  points: number
  withSomeone: boolean
  inNature: boolean
  place: string
  routeSeed: number
  /** emoji (placeholder) albo data:/http URL zdjęcia */
  photos: string[]
}

const KEY = 'ss-walks'

const sample: SavedWalk[] = [
  {
    id: 's1',
    dateLabel: 'Wczoraj • 18:10',
    durationSec: 2730,
    steps: 5120,
    points: 132,
    withSomeone: true,
    inNature: true,
    place: 'Bulwar Nadmorski, Gdynia',
    routeSeed: 4821,
    photos: ['🌅', '🏖️'],
  },
  {
    id: 's2',
    dateLabel: 'Pon • 7:40',
    durationSec: 1980,
    steps: 3460,
    points: 74,
    withSomeone: false,
    inNature: true,
    place: 'Plaża Brzeźno',
    routeSeed: 1390,
    photos: ['🌊'],
  },
]

export function getWalks(): SavedWalk[] {
  try {
    const v = localStorage.getItem(KEY)
    return v ? (JSON.parse(v) as SavedWalk[]) : sample
  } catch {
    return sample
  }
}

export function addWalk(w: SavedWalk): SavedWalk[] {
  const list = [w, ...getWalks()]
  try {
    localStorage.setItem(KEY, JSON.stringify(list))
  } catch {
    /* quota — ignorujemy w demo */
  }
  return list
}
