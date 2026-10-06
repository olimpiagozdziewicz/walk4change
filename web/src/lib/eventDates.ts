/**
 * Formatowanie dat wydarzeń z kalendarza IGTSF. Serwer oddaje czas lokalny
 * (Europe/Warsaw) bez strefy: `YYYY-MM-DDTHH:MM:SS` — parsujemy ręcznie, żeby
 * nie zależeć od strefy telefonu ani od ICU.
 */

const MONTHS = ['sty', 'lut', 'mar', 'kwi', 'maj', 'cze', 'lip', 'sie', 'wrz', 'paź', 'lis', 'gru']
const WEEKDAYS = ['nd', 'pn', 'wt', 'śr', 'cz', 'pt', 'sb']

interface LocalDateTime {
  y: number
  m: number
  d: number
  hh: string
  mm: string
}

function parseLocal(s: string | null): LocalDateTime | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(s ?? '')
  if (!match) return null
  const [, y, m, d, hh, mm] = match
  const month = Number(m)
  if (month < 1 || month > 12) return null
  return { y: Number(y), m: month, d: Number(d), hh, mm }
}

function dayLabel(t: LocalDateTime): string {
  const weekday = WEEKDAYS[new Date(Date.UTC(t.y, t.m - 1, t.d)).getUTCDay()]
  return `${weekday} ${t.d} ${MONTHS[t.m - 1]}`
}

const time = (t: LocalDateTime) => `${t.hh}:${t.mm}`
const sameDay = (a: LocalDateTime, b: LocalDateTime) => a.y === b.y && a.m === b.m && a.d === b.d

/** np. „pn 6 paź • 09:00–13:00”, „pn 6 paź, 10:00 – śr 8 paź”, „sb 11 paź • cały dzień”. */
export function formatEventWhen(start: string, end: string | null, allDay: boolean): string {
  const s = parseLocal(start)
  if (!s) return start
  const e = parseLocal(end)
  if (e && !sameDay(s, e)) {
    return `${dayLabel(s)}${allDay ? '' : `, ${time(s)}`} – ${dayLabel(e)}`
  }
  if (allDay) return `${dayLabel(s)} • cały dzień`
  if (e && time(e) !== time(s)) return `${dayLabel(s)} • ${time(s)}–${time(e)}`
  return `${dayLabel(s)} • ${time(s)}`
}

/** „Centrum · Kraków”; bez powtórek; null, gdy brak miejsca (np. online). */
export function formatEventPlace(venue: string | null, city: string | null): string | null {
  const parts = [venue, city].filter((p): p is string => !!p && p.trim() !== '')
  const unique = parts.filter((p, i) => parts.findIndex((q) => q.toLowerCase() === p.toLowerCase()) === i)
  return unique.length > 0 ? unique.join(' · ') : null
}
