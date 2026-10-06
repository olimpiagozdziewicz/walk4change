import { useEffect, useState } from 'react'
import { motion } from 'motion/react'
import { MapPin, CalendarDots, UsersThree, CalendarHeart, ArrowSquareOut, Leaf } from '@phosphor-icons/react'
import { ScreenHeader, Card, Pill, PrimaryButton, SoonBadge, DemoBanner } from '../components/ui'
import { useMode } from '../lib/mode'
import { api, type EventItem, type EventType, type CalendarEvent } from '../lib/api'
import { formatEventWhen, formatEventPlace } from '../lib/eventDates'
import { IGTSF_PARTNER } from '../lib/partners'

const typeMeta: Record<EventType, { emoji: string; label: string; tone: 'sea' | 'leaf' | 'sand' }> = {
  cleanup: { emoji: '🧹', label: 'Sprzątanie', tone: 'sea' },
  planting: { emoji: '🌳', label: 'Sadzenie', tone: 'leaf' },
  social: { emoji: '🚶', label: 'Spacer', tone: 'sand' },
  baltic: { emoji: '🌊', label: 'Pro-Bałtyk', tone: 'sea' },
}

export function Events() {
  const { mode } = useMode()
  const isTeam = mode === 'team'

  return (
    <div>
      <ScreenHeader
        title={isTeam ? 'Eventy firmowe' : 'Eventy'}
        icon={<CalendarHeart size={22} />}
        subtitle={isTeam ? 'Integracja, CSR i akcje eko dla zespołów.' : 'Wydarzenia ekologiczne w Polsce i online.'}
      />
      {isTeam ? <CorporateEvents /> : <PartnerCalendar />}
    </div>
  )
}

/** Prawdziwy kalendarz partnera (backend /events → igtsf.com, cache 30 min). */
function PartnerCalendar() {
  const [state, setState] = useState<'loading' | 'ready' | 'error'>('loading')
  const [events, setEvents] = useState<CalendarEvent[]>([])

  useEffect(() => {
    let alive = true
    api
      .getCalendarEvents()
      .then((list) => { if (alive) { setEvents(list); setState('ready') } })
      .catch(() => { if (alive) setState('error') })
    return () => { alive = false }
  }, [])

  return (
    <div className="space-y-3 px-5 pt-2">
      <p className="px-1 text-xs font-bold text-muted">
        Kalendarz:{' '}
        <a href={IGTSF_PARTNER.siteUrl} target="_blank" rel="noopener" className="text-sea underline">
          {IGTSF_PARTNER.name}
        </a>
      </p>

      {state === 'loading' && <p className="px-1 text-sm font-semibold text-muted">Wczytywanie wydarzeń…</p>}

      {state === 'error' && (
        <Card className="p-4">
          <p className="text-sm text-muted">Nie udało się wczytać kalendarza. Spróbuj za chwilę.</p>
        </Card>
      )}

      {state === 'ready' && events.length === 0 && (
        <Card className="p-4">
          <p className="text-sm text-muted">Brak nadchodzących wydarzeń. Zajrzyj później.</p>
          <a
            href={IGTSF_PARTNER.calendarUrl}
            target="_blank"
            rel="noopener"
            className="mt-2 inline-flex items-center gap-1 text-sm font-bold text-sea"
          >
            Kalendarz na igtsf.com <ArrowSquareOut size={14} />
          </a>
        </Card>
      )}

      {events.map((e, i) => {
        const place = formatEventPlace(e.venue, e.city)
        return (
          <motion.div
            key={e.id}
            initial={{ opacity: 0, y: 14 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: Math.min(i, 6) * 0.06 }}
          >
            <Card className="overflow-hidden p-4">
              <div className="flex items-start gap-3">
                <EventThumb src={e.image} />
                <div className="min-w-0 flex-1">
                  <div className="font-display text-[17px] font-bold leading-tight text-ink [overflow-wrap:anywhere]">
                    {e.title}
                  </div>
                  <div className="mt-1.5 flex flex-col gap-0.5 text-xs font-bold text-muted">
                    <span className="inline-flex items-center gap-1">
                      <CalendarDots size={13} className="shrink-0" /> {formatEventWhen(e.start, e.end, e.allDay)}
                    </span>
                    {place && (
                      <span className="inline-flex items-center gap-1">
                        <MapPin size={13} className="shrink-0" /> {place}
                      </span>
                    )}
                  </div>
                </div>
              </div>
              {e.description && <p className="mt-2.5 line-clamp-3 text-sm text-muted">{e.description}</p>}
              {e.url && (
                <a
                  href={e.url}
                  target="_blank"
                  rel="noopener"
                  className="mt-3 inline-flex items-center gap-1 text-sm font-bold text-sea"
                >
                  Więcej na igtsf.com <ArrowSquareOut size={14} />
                </a>
              )}
            </Card>
          </motion.div>
        )
      })}
    </div>
  )
}

function EventThumb({ src }: { src: string | null }) {
  const [failed, setFailed] = useState(false)
  if (!src || failed) {
    return (
      <div className="grid h-14 w-14 shrink-0 place-items-center rounded-2xl bg-gradient-to-br from-sea/12 to-leaf/15 text-[#2f7a45]">
        <Leaf size={26} />
      </div>
    )
  }
  return (
    <img
      src={src}
      alt=""
      loading="lazy"
      referrerPolicy="no-referrer"
      onError={() => setFailed(true)}
      className="h-14 w-14 shrink-0 rounded-2xl object-cover"
    />
  )
}

/** Tryb firmowy: dalej podgląd (demo), jak dotąd. */
function CorporateEvents() {
  const [events, setEvents] = useState<EventItem[]>([])

  useEffect(() => {
    api.getCorporateEvents().then(setEvents)
  }, [])

  return (
    <div className="space-y-3 px-5 pt-2">
      <DemoBanner>
        Wkrótce — tu pojawią się wydarzenia firmowe: sprzątanie plaży, sadzenie drzew, wspólne spacery. Na razie to
        podgląd, jak będą wyglądać.
      </DemoBanner>
      {events.map((e, i) => {
        const meta = typeMeta[e.type]
        return (
          <motion.div key={e.id} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.08 }}>
            <Card className="overflow-hidden">
              <div className="flex items-center gap-3 p-4 pb-3">
                <div className="grid h-12 w-12 place-items-center rounded-2xl bg-gradient-to-br from-sea/12 to-leaf/12 text-2xl">
                  {meta.emoji}
                </div>
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <Pill tone={meta.tone}>{meta.label}</Pill>
                    <span className="text-xs font-bold text-[#2f7a45]">+{e.points} pkt</span>
                  </div>
                  <div className="mt-1 font-display text-[17px] font-bold leading-tight text-ink">{e.title}</div>
                </div>
                <SoonBadge />
              </div>
              <div className="flex flex-wrap gap-x-4 gap-y-1 px-4 text-xs font-bold text-muted">
                <span className="inline-flex items-center gap-1">
                  <CalendarDots size={13} /> {e.date}
                </span>
                <span className="inline-flex items-center gap-1">
                  <MapPin size={13} /> {e.place}
                </span>
                <span className="inline-flex items-center gap-1">
                  <UsersThree size={13} /> {e.peopleCount} osób
                </span>
              </div>
              <div className="p-4 pt-3">
                <PrimaryButton disabled className="w-full py-2.5 text-sm">
                  Wkrótce
                </PrimaryButton>
              </div>
            </Card>
          </motion.div>
        )
      })}
    </div>
  )
}
