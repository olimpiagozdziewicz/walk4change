import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { motion, AnimatePresence } from 'motion/react'
import { Play, Square, UsersThree, Leaf, Trophy, ArrowRight } from '@phosphor-icons/react'
import { ScreenHeader, Card, PrimaryButton, SoftButton, Pill } from '../components/ui'
import { FootstepTrail } from '../components/Footsteps'
import { computeWalkPoints } from '../lib/api'

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
    setPhase('active')
  }
  const stop = () => {
    if (timer.current) window.clearInterval(timer.current)
    setPhase('summary')
  }
  const reset = () => setPhase('idle')

  return (
    <div>
      <ScreenHeader title="Spacer" emoji="👣" subtitle="Każdy krok to punkty. Nad wodą i we dwoje — jeszcze więcej." />

      <div className="px-5">
        <AnimatePresence mode="wait">
          {/* ── IDLE ── */}
          {phase === 'idle' && (
            <motion.div key="idle" initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
              <Card className="relative mt-2 overflow-hidden p-6 text-center">
                <div className="pointer-events-none absolute inset-x-0 bottom-2 flex justify-center opacity-50">
                  <FootstepTrail count={6} color="#0f8b8d" />
                </div>
                <div className="relative">
                  <div className="mx-auto mb-4 grid h-20 w-20 place-items-center rounded-full bg-gradient-to-br from-sea/15 to-leaf/15 text-4xl">
                    🚶‍♀️
                  </div>
                  <h2 className="font-display text-2xl font-bold text-ink">Gotowa na spacer?</h2>
                  <p className="mx-auto mt-2 max-w-[260px] text-sm text-muted">
                    Włącz spacer, a SeaSteps policzy kroki, czas i punkty na żywo.
                  </p>
                </div>
              </Card>

              <ToggleRow withSomeone={withSomeone} setWithSomeone={setWithSomeone} inNature={inNature} setInNature={setInNature} />

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

              <ToggleRow withSomeone={withSomeone} setWithSomeone={setWithSomeone} inNature={inNature} setInNature={setInNature} />

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

                <p className="mt-4 text-sm font-bold text-[#2f7a45]">🦭 Jesteś coraz bliżej adopcji foki!</p>
              </Card>

              <div className="mt-4 grid grid-cols-2 gap-3">
                <SoftButton onClick={reset}>Jeszcze raz</SoftButton>
                <PrimaryButton onClick={() => nav('/community')}>
                  Społeczność <ArrowRight size={18} />
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
}: {
  withSomeone: boolean
  setWithSomeone: (v: boolean) => void
  inNature: boolean
  setInNature: (v: boolean) => void
}) {
  return (
    <div className="mt-4 grid grid-cols-2 gap-3">
      <Toggle active={inNature} onClick={() => setInNature(!inNature)} icon={<Leaf size={18} />} label="W naturze" hint="×3" />
      <Toggle active={withSomeone} onClick={() => setWithSomeone(!withSomeone)} icon={<UsersThree size={18} />} label="Z kimś" hint="×1.5" />
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
  hint: string
}) {
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-2.5 rounded-2xl px-4 py-3 text-left text-sm font-bold transition active:scale-[0.97] ${
        active ? 'bg-gradient-to-br from-leaf/20 to-sea/15 text-deep ring-1 ring-leaf/40' : 'glass text-muted'
      }`}
    >
      <span className={`grid h-9 w-9 place-items-center rounded-full ${active ? 'bg-white text-leaf' : 'bg-white/60 text-muted'}`}>
        {icon}
      </span>
      <span className="flex-1">
        {label}
        <span className="ml-1 text-xs opacity-70">{hint}</span>
      </span>
    </button>
  )
}
