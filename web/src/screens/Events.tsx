import { useEffect, useState } from 'react'
import { motion } from 'motion/react'
import { MapPin, CalendarDays, Users } from 'lucide-react'
import { ScreenHeader, Card, Pill, PrimaryButton } from '../components/ui'
import { useMode } from '../lib/mode'
import { api, type EventItem, type EventType } from '../lib/api'

const typeMeta: Record<EventType, { emoji: string; label: string; tone: 'sea' | 'leaf' | 'sand' }> = {
  cleanup: { emoji: '🧹', label: 'Sprzątanie', tone: 'sea' },
  planting: { emoji: '🌳', label: 'Sadzenie', tone: 'leaf' },
  social: { emoji: '🚶', label: 'Spacer', tone: 'sand' },
  baltic: { emoji: '🌊', label: 'Pro-Bałtyk', tone: 'sea' },
}

export function Events() {
  const { mode } = useMode()
  const isTeam = mode === 'team'
  const [events, setEvents] = useState<EventItem[]>([])
  const [joined, setJoined] = useState<Record<string, boolean>>({})

  useEffect(() => {
    ;(isTeam ? api.getCorporateEvents() : api.getEvents()).then(setEvents)
  }, [isTeam])

  return (
    <div>
      <ScreenHeader
        title={isTeam ? 'Eventy firmowe' : 'Eventy'}
        emoji="🌍"
        subtitle={isTeam ? 'Integracja, CSR i akcje eko dla zespołów.' : 'Akcje społeczne i eko dla Trójmiasta i Bałtyku.'}
      />

      <div className="space-y-3 px-5 pt-2">
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
                </div>
                <div className="flex flex-wrap gap-x-4 gap-y-1 px-4 text-xs font-bold text-muted">
                  <span className="inline-flex items-center gap-1">
                    <CalendarDays size={13} /> {e.date}
                  </span>
                  <span className="inline-flex items-center gap-1">
                    <MapPin size={13} /> {e.place}
                  </span>
                  <span className="inline-flex items-center gap-1">
                    <Users size={13} /> {e.peopleCount} osób
                  </span>
                </div>
                <div className="p-4 pt-3">
                  <PrimaryButton onClick={() => setJoined((j) => ({ ...j, [e.id]: !j[e.id] }))} className="w-full py-2.5 text-sm">
                    {joined[e.id] ? '✓ Zapisana — do zobaczenia!' : 'Dołączam'}
                  </PrimaryButton>
                </div>
              </Card>
            </motion.div>
          )
        })}
      </div>
    </div>
  )
}
