import { useEffect, useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { motion } from 'motion/react'
import { Footprints, CalendarHeart, Recycle, GearSix, PencilSimple, Check } from '@phosphor-icons/react'
import { Card, Pill } from '../components/ui'
import { Glyph } from '../components/Glyph'
import { FootstepTrail } from '../components/Footsteps'
import { api, INTEREST_OPTIONS, type Profile as ProfileT } from '../lib/api'
import { getInterests, saveInterests } from '../lib/interests'
import { getGender, saveGender, type Gender } from '../lib/settings'

export function Profile() {
  const nav = useNavigate()
  const [p, setP] = useState<ProfileT | null>(null)
  const [interests, setInterests] = useState<string[]>(getInterests())
  const [editing, setEditing] = useState(false)
  const [gender, setGender] = useState<Gender>(getGender())

  const pickGender = (g: Gender) => {
    setGender(g)
    saveGender(g)
  }

  useEffect(() => {
    api.getProfile().then(setP)
  }, [])

  const toggle = (tag: string) =>
    setInterests((cur) => (cur.includes(tag) ? cur.filter((t) => t !== tag) : [...cur, tag]))

  const finishEdit = () => {
    saveInterests(interests)
    setEditing(false)
  }

  if (!p) return null

  return (
    <div>
      <div className="px-5 pt-5">
        {/* identity card */}
        <Card className="relative overflow-hidden p-6 text-center">
          <div className="pointer-events-none absolute -right-2 top-2 opacity-30">
            <FootstepTrail count={5} color="#0f8b8d" />
          </div>
          <button onClick={() => nav('/')} className="absolute right-4 top-4 text-muted" aria-label="Ustawienia">
            <GearSix size={20} />
          </button>
          <motion.div
            initial={{ scale: 0 }}
            animate={{ scale: 1 }}
            transition={{ type: 'spring' }}
            className="mx-auto grid h-24 w-24 place-items-center rounded-full bg-gradient-to-br from-sea to-leaf font-display text-4xl font-bold text-white shadow-[0_16px_30px_rgba(15,139,141,0.3)]"
          >
            {p.name.charAt(0)}
          </motion.div>
          <h2 className="mt-3 font-display text-2xl font-bold text-ink">{p.name}</h2>

          <div className="mt-2 inline-flex rounded-full bg-white/70 p-1 text-xs font-bold ring-1 ring-white/60">
            <button
              onClick={() => pickGender('f')}
              className={`rounded-full px-3 py-1 transition ${gender === 'f' ? 'bg-gradient-to-br from-sea to-deep text-white shadow' : 'text-muted'}`}
            >
              Ona
            </button>
            <button
              onClick={() => pickGender('m')}
              className={`rounded-full px-3 py-1 transition ${gender === 'm' ? 'bg-gradient-to-br from-sea to-deep text-white shadow' : 'text-muted'}`}
            >
              On
            </button>
          </div>

          {/* interests */}
          <div className="mt-4 flex items-center justify-center gap-2">
            <span className="text-xs font-bold uppercase tracking-wide text-muted">Zainteresowania</span>
            <button
              onClick={() => (editing ? finishEdit() : setEditing(true))}
              className="inline-flex items-center gap-1 rounded-full bg-sea/10 px-2.5 py-1 text-xs font-bold text-deep transition active:scale-95"
            >
              {editing ? (
                <>
                  <Check size={13} /> Gotowe
                </>
              ) : (
                <>
                  <PencilSimple size={13} /> Edytuj
                </>
              )}
            </button>
          </div>

          {editing ? (
            <div className="mt-3 flex flex-wrap justify-center gap-1.5">
              {INTEREST_OPTIONS.map((tag) => {
                const on = interests.includes(tag)
                return (
                  <button
                    key={tag}
                    onClick={() => toggle(tag)}
                    className={`rounded-full px-3 py-1 text-xs font-bold transition active:scale-95 ${
                      on ? 'bg-gradient-to-br from-sea to-leaf text-white shadow' : 'bg-white/70 text-muted'
                    }`}
                  >
                    {tag}
                  </button>
                )
              })}
            </div>
          ) : (
            <div className="mt-3 flex flex-wrap justify-center gap-1.5">
              {interests.length ? (
                interests.map((i) => (
                  <Pill key={i} tone="sea">
                    {i}
                  </Pill>
                ))
              ) : (
                <span className="text-sm text-muted">Dodaj zainteresowania, by dopasować ludzi do spacerów 🌊</span>
              )}
            </div>
          )}
          {!editing && (
            <p className="mt-3 text-xs text-muted">Na podstawie zainteresowań proponujemy Ci ludzi do wspólnych spacerów.</p>
          )}
        </Card>

        {/* stats */}
        <div className="mt-4 grid grid-cols-3 gap-3">
          <StatCard icon={<Footprints size={22} />} value={p.stats.walks} label="spacerów" onClick={() => nav('/history')} />
          <StatCard icon={<CalendarHeart size={22} />} value={p.stats.events} label="eventów" />
          <StatCard icon={<Recycle size={22} />} value={p.stats.ecoReports} label="eko-zgłoszeń" />
        </div>

        {/* badges */}
        <h2 className="mb-3 mt-6 font-display text-lg font-bold text-ink">Odznaki</h2>
        <div className="grid grid-cols-2 gap-3">
          {p.badges.map((b, i) => (
            <motion.div key={b.id} initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.07 }}>
              <Card className="flex items-center gap-3 p-3.5">
                <div className="grid h-11 w-11 place-items-center rounded-2xl bg-sand/20 text-[#c8761b]">
                  <Glyph k={b.iconKey} size={22} />
                </div>
                <span className="text-sm font-bold leading-tight text-ink">{b.label}</span>
              </Card>
            </motion.div>
          ))}
        </div>

      </div>
    </div>
  )
}

function StatCard({ icon, value, label, onClick }: { icon: ReactNode; value: number; label: string; onClick?: () => void }) {
  return (
    <Card className="p-4 text-center" onClick={onClick}>
      <div className="mx-auto mb-1.5 flex h-9 w-9 items-center justify-center rounded-full bg-sea/10 text-sea">{icon}</div>
      <div className="font-display text-2xl font-bold text-deep">{value}</div>
      <div className="text-[11px] font-bold text-muted">{label}</div>
    </Card>
  )
}
