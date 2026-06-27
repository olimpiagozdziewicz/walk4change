import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { motion, AnimatePresence } from 'motion/react'
import { Play, Square, UsersThree, Leaf, Trophy, Camera, Check, ClockCounterClockwise, Footprints, PersonSimpleWalk, HandHeart, PawPrint, Lightbulb } from '@phosphor-icons/react'
import { ScreenHeader, Card, PrimaryButton, SoftButton, Pill } from '../components/ui'
import { FootstepTrail } from '../components/Footsteps'
import { Celebrate } from '../components/Celebrate'
import { computeWalkPoints } from '../lib/api'
import { addWalk } from '../lib/walks'

const PLACES = ['Bulwar Nadmorski', 'Plaża Brzeźno', 'Park Oliwski', 'Molo w Orłowie', 'Plaża Stogi']

const PLACE_FACTS: Record<string, string> = {
  'Bulwar Nadmorski': 'Gdyński bulwar ma ok. 1,5 km i jest jednym z ulubionych miejsc na zachód słońca nad Zatoką.',
  'Plaża Brzeźno': 'Molo w Brzeźnie ma 136 m długości, a plaża to siedlisko rzadkich ptaków siewkowych.',
  'Park Oliwski': 'Park Oliwski powstał w XVIII w. i kryje ponad 130 gatunków drzew i krzewów.',
  'Molo w Orłowie': 'Drewniane molo w Orłowie z 1934 r. to jedno z najbardziej malowniczych miejsc Trójmiasta.',
  'Plaża Stogi': 'Stogi to jedna z najszerszych plaż Gdańska — szeroka na ponad 100 m miejscami.',
}
const factFor = (place: string) => PLACE_FACTS[place] ?? 'Bałtyk to jedno z najmłodszych mórz świata — ma ok. 12 tys. lat.'

type Phase = 'idle' | 'active' | 'summary'

function fmt(sec: number) {
  const m = Math.floor(sec / 60)
  const s = sec % 60
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
}

export function Walk() {
  const nav = useNavigate()
  const [phase, setPhase] = useState<Phase>('idle')
  const [sec, setSec] = useState(0)
  const [steps, setSteps] = useState(0)
  const [withSomeone, setWithSomeone] = useState(false)
  const [inNature, setInNature] = useState(true)
  const [withDog, setWithDog] = useState(false)
  const [place, setPlace] = useState('')
  const [photos, setPhotos] = useState<string[]>([])
  const fileRef = useRef<HTMLInputElement>(null)
  const timer = useRef<number | null>(null)

  useEffect(() => {
    if (phase === 'active') {
      timer.current = window.setInterval(() => {
        setSec((s) => s + 1)
        setSteps((s) => s + Math.floor(8 + Math.random() * 6)) // ~8-14 kroków/s
      }, 1000)
    }
    return () => {
      if (timer.current) window.clearInterval(timer.current)
    }
  }, [phase])

  const { base, total, multiplier } = computeWalkPoints({ steps, withSomeone, inNature })

  const start = () => {
    setSec(0)
    setSteps(0)
    setPhotos([])
    setPhase('active')
  }
  const stop = () => {
    if (timer.current) window.clearInterval(timer.current)
    setPlace(PLACES[Math.floor(Math.random() * PLACES.length)])
    setPhase('summary')
  }
  const reset = () => {
    setPhotos([])
    setPhase('idle')
  }

  const onFile = (e: React.ChangeEvent<HTMLInputElement>) => {
    const f = e.target.files?.[0]
    if (!f) return
    const r = new FileReader()
    r.onload = () => setPhotos((p) => [...p, String(r.result)])
    r.readAsDataURL(f)
  }

  const save = () => {
    const now = new Date()
    const hh = String(now.getHours()).padStart(2, '0')
    const mm = String(now.getMinutes()).padStart(2, '0')
    addWalk({
      id: 'w' + now.getTime(),
      dateLabel: `Dziś • ${hh}:${mm}`,
      durationSec: sec,
      steps,
      points: total,
      withSomeone,
      inNature,
      withDog,
      place: place || PLACES[0],
      routeSeed: Math.floor(Math.random() * 100000),
      photos,
    })
    nav('/history')
  }

  return (
    <div>
      <ScreenHeader title="Spacer" icon={<Footprints size={22} />} subtitle="Każdy krok to punkty. Nad wodą i we dwoje — jeszcze więcej." />

      <div className="px-5">
        <AnimatePresence mode="wait">
          {/* ── IDLE ── */}
          {phase === 'idle' && (
            <motion.div key="idle" initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
              <button
                onClick={() => nav('/history')}
                className="mb-3 inline-flex w-full items-center justify-center gap-2 rounded-2xl glass py-2.5 text-sm font-bold text-deep transition active:scale-[0.98]"
              >
                <ClockCounterClockwise size={16} /> Moje spacery
              </button>
              <Card className="relative mt-2 overflow-hidden p-6 text-center">
                <div className="pointer-events-none absolute inset-x-0 bottom-2 flex justify-center opacity-50">
                  <FootstepTrail count={6} color="#0f8b8d" />
                </div>
                <div className="relative">
                  <div className="mx-auto mb-4 grid h-20 w-20 place-items-center rounded-full bg-gradient-to-br from-sea/15 to-leaf/15 text-sea">
                    <PersonSimpleWalk size={40} weight="fill" />
                  </div>
                  <h2 className="font-display text-2xl font-bold text-ink">Gotowa na spacer?</h2>
                  <p className="mx-auto mt-2 max-w-[260px] text-sm text-muted">
                    Włącz spacer, a SeaSteps policzy kroki, czas i punkty na żywo.
                  </p>
                </div>
              </Card>

              <ToggleRow withSomeone={withSomeone} setWithSomeone={setWithSomeone} inNature={inNature} setInNature={setInNature} withDog={withDog} setWithDog={setWithDog} />

              <PrimaryButton onClick={start} className="mt-5 w-full py-4 text-base">
                <Play size={20} weight="fill" color="white" /> Rozpocznij spacer
              </PrimaryButton>
            </motion.div>
          )}

          {/* ── ACTIVE ── */}
          {phase === 'active' && (
            <motion.div key="active" initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
              <Card className="relative mt-2 overflow-hidden p-6">
                <div className="pointer-events-none absolute right-3 top-0 opacity-70">
                  <FootstepTrail count={7} color="#58b86c" />
                </div>
                <div className="text-center">
                  <Pill tone="leaf">● na żywo</Pill>
                  <div className="mt-3 font-display text-6xl font-bold tabular-nums tracking-tight text-deep">{fmt(sec)}</div>
                  <div className="mt-1 text-sm font-bold text-muted">czas spaceru</div>
                </div>

                <div className="mt-6 grid grid-cols-2 gap-3">
                  <Stat label="kroki" value={steps.toLocaleString('pl-PL')} />
                  <Stat label="punkty" value={String(total)} accent />
                </div>

                {multiplier > 1 && (
                  <div className="mt-3 flex justify-center gap-2">
                    {inNature && (
                      <Pill tone="leaf">
                        <Leaf size={12} /> natura ×3
                      </Pill>
                    )}
                    {withSomeone && (
                      <Pill tone="sea">
                        <UsersThree size={12} /> z kimś ×1.5
                      </Pill>
                    )}
                  </div>
                )}
              </Card>

              <ToggleRow withSomeone={withSomeone} setWithSomeone={setWithSomeone} inNature={inNature} setInNature={setInNature} withDog={withDog} setWithDog={setWithDog} />

              <button
                onClick={stop}
                className="mt-5 inline-flex w-full items-center justify-center gap-2 rounded-2xl border border-[#e6b4b4] bg-white/80 py-4 text-base font-bold text-[#c0504d] transition active:scale-[0.97]"
              >
                <Square size={18} weight="fill" color="#c0504d" /> Zakończ spacer
              </button>
            </motion.div>
          )}

          {/* ── SUMMARY ── */}
          {phase === 'summary' && (
            <motion.div key="summary" initial={{ opacity: 0, scale: 0.96 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0 }}>
              <Celebrate />
              <Card className="mt-2 overflow-hidden p-6 text-center">
                <motion.div
                  initial={{ scale: 0, rotate: -20 }}
                  animate={{ scale: 1, rotate: 0 }}
                  transition={{ type: 'spring', delay: 0.1 }}
                  className="mx-auto mb-3 grid h-20 w-20 place-items-center rounded-full bg-gradient-to-br from-sea to-leaf text-4xl shadow-[0_16px_30px_rgba(15,139,141,0.3)]"
                >
                  <Trophy size={36} color="white" />
                </motion.div>
                <h2 className="font-display text-2xl font-bold text-ink">Brawo, spacer zaliczony!</h2>
                <p className="mt-1 text-sm text-muted">
                  {fmt(sec)} • {steps.toLocaleString('pl-PL')} kroków
                </p>

                <div className="mt-5 rounded-3xl bg-gradient-to-br from-sea/10 to-leaf/10 p-5">
                  <div className="font-display text-5xl font-bold text-sea">+{total}</div>
                  <div className="text-sm font-bold text-muted">punktów zdobytych</div>
                  {multiplier > 1 && (
                    <div className="mt-2 text-xs font-bold text-deep">
                      {base} bazowe × {multiplier.toFixed(2).replace('.00', '')} bonus
                    </div>
                  )}
                </div>

                <p className="mt-4 inline-flex items-center justify-center gap-1.5 text-sm font-bold text-[#2f7a45]">
                  <HandHeart size={16} weight="fill" /> Jesteś coraz bliżej adopcji foki!
                </p>
              </Card>

              {/* ciekawostka o miejscu */}
              <Card className="mt-4 flex items-start gap-3 p-4">
                <span className="grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-sand/25 text-[#c8761b]">
                  <Lightbulb size={18} weight="fill" />
                </span>
                <div>
                  <div className="text-xs font-bold uppercase tracking-wide text-muted">Czy wiesz, że… • {place}</div>
                  <p className="mt-0.5 text-sm leading-snug text-ink">{factFor(place)}</p>
                </div>
              </Card>

              {/* dodatkowe bonusy */}
              {withDog && (
                <div className="mt-3 flex justify-center">
                  <Pill tone="leaf">
                    <PawPrint size={12} /> spacer z psem
                  </Pill>
                </div>
              )}

              {/* zdjęcia z trasy */}
              <div className="mt-4">
                <input ref={fileRef} type="file" accept="image/*" className="hidden" onChange={onFile} />
                <div className="no-scrollbar flex gap-2 overflow-x-auto">
                  <button
                    onClick={() => fileRef.current?.click()}
                    className="grid h-20 w-20 shrink-0 place-items-center rounded-2xl border border-dashed border-sea/40 bg-white/50 text-deep transition active:scale-95"
                  >
                    <Camera size={22} />
                  </button>
                  {photos.map((p, i) => (
                    <img key={i} src={p} alt="" className="h-20 w-20 shrink-0 rounded-2xl object-cover" />
                  ))}
                </div>
                <p className="mt-1.5 text-center text-xs text-muted">Dodaj zdjęcia z trasy 📸</p>
              </div>

              <div className="mt-4 grid grid-cols-2 gap-3">
                <SoftButton onClick={reset}>Jeszcze raz</SoftButton>
                <PrimaryButton onClick={save}>
                  <Check size={18} /> Zapisz spacer
                </PrimaryButton>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  )
}

function Stat({ label, value, accent }: { label: string; value: string; accent?: boolean }) {
  return (
    <div className="rounded-2xl bg-white/60 p-4 text-center">
      <div className={`font-display text-3xl font-bold tabular-nums ${accent ? 'text-sea' : 'text-ink'}`}>{value}</div>
      <div className="text-xs font-bold text-muted">{label}</div>
    </div>
  )
}

function ToggleRow({
  withSomeone,
  setWithSomeone,
  inNature,
  setInNature,
  withDog,
  setWithDog,
}: {
  withSomeone: boolean
  setWithSomeone: (v: boolean) => void
  inNature: boolean
  setInNature: (v: boolean) => void
  withDog: boolean
  setWithDog: (v: boolean) => void
}) {
  return (
    <div className="mt-4 grid grid-cols-3 gap-2.5">
      <Toggle active={inNature} onClick={() => setInNature(!inNature)} icon={<Leaf size={20} />} label="Natura" hint="×3" />
      <Toggle active={withSomeone} onClick={() => setWithSomeone(!withSomeone)} icon={<UsersThree size={20} />} label="Z kimś" hint="×1.5" />
      <Toggle active={withDog} onClick={() => setWithDog(!withDog)} icon={<PawPrint size={20} />} label="Z psem" />
    </div>
  )
}

function Toggle({
  active,
  onClick,
  icon,
  label,
  hint,
}: {
  active: boolean
  onClick: () => void
  icon: ReactNode
  label: string
  hint?: string
}) {
  return (
    <button
      onClick={onClick}
      className={`flex flex-col items-center gap-1.5 rounded-2xl px-2 py-3 text-center text-xs font-bold transition active:scale-95 ${
        active ? 'bg-gradient-to-br from-leaf/20 to-sea/15 text-deep ring-1 ring-leaf/40' : 'glass text-muted'
      }`}
    >
      <span className={`grid h-9 w-9 place-items-center rounded-full ${active ? 'bg-white text-leaf' : 'bg-white/60 text-muted'}`}>
        {icon}
      </span>
      <span>
        {label}
        {hint && <span className="ml-0.5 opacity-70">{hint}</span>}
      </span>
    </button>
  )
}
