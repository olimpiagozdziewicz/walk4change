// Trwały ślad aktywnego spaceru (spec 2026-10-05). Android potrafi zabić proces
// apki w tle — bez tego zapisu spacer „znikał”, choć sesja na serwerze trwała.
// Ekran Spacer wznawia sesję z tego zapisu, jeśli serwer potwierdzi, że żyje.

const ACTIVE_KEY = 'ss-active-walk'
const JOIN_KEY = 'ss-pending-join'

export interface ActiveWalk {
  sessionId: string
  joinCode: string | null
  /** epoch ms startu spaceru (zegar licznika czasu) */
  startedAt: number
  /** moje metry i punkty z ostatniego ocenionego pingu */
  meters: number
  points: number
}

export function saveActiveWalk(w: ActiveWalk): void {
  try {
    localStorage.setItem(ACTIVE_KEY, JSON.stringify(w))
  } catch {
    /* brak storage — spacer działa, tylko bez wznowienia */
  }
}

export function loadActiveWalk(): ActiveWalk | null {
  try {
    const raw = localStorage.getItem(ACTIVE_KEY)
    if (!raw) return null
    const w = JSON.parse(raw) as Partial<ActiveWalk>
    if (typeof w.sessionId !== 'string' || typeof w.startedAt !== 'number') return null
    return {
      sessionId: w.sessionId,
      joinCode: typeof w.joinCode === 'string' ? w.joinCode : null,
      startedAt: w.startedAt,
      meters: typeof w.meters === 'number' ? w.meters : 0,
      points: typeof w.points === 'number' ? w.points : 0,
    }
  } catch {
    return null
  }
}

export function clearActiveWalk(): void {
  try {
    localStorage.removeItem(ACTIVE_KEY)
  } catch {
    /* ignore */
  }
}

/** Kod dołączenia do spaceru (8 znaków A-Z0-9), jak generuje backend. */
export function normalizeJoinCode(raw: string | null | undefined): string | null {
  const code = (raw ?? '').trim().toUpperCase()
  return /^[A-Z0-9]{4,12}$/.test(code) ? code : null
}

/** Link do dołączenia, zakodowany w QR. Otwiera apkę (App Link) albo PWA. */
export function joinUrl(code: string): string {
  return `https://seasteps.pl/app/walk?join=${encodeURIComponent(code)}`
}

// Kod z linku/QR musi przetrwać przekierowanie na logowanie — stąd storage.
/** Zdarzenie „nowy kod z QR” — dla już otwartego ekranu Spacer. */
export const PENDING_JOIN_EVENT = 'ss-pending-join'

export function setPendingJoin(code: string): void {
  try {
    sessionStorage.setItem(JOIN_KEY, code)
  } catch {
    /* ignore */
  }
  window.dispatchEvent(new Event(PENDING_JOIN_EVENT))
}

export function takePendingJoin(): string | null {
  try {
    const code = sessionStorage.getItem(JOIN_KEY)
    sessionStorage.removeItem(JOIN_KEY)
    return normalizeJoinCode(code)
  } catch {
    return null
  }
}

export function hasPendingJoin(): boolean {
  try {
    return sessionStorage.getItem(JOIN_KEY) != null
  } catch {
    return false
  }
}
