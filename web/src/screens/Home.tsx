import { useEffect, useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { motion } from 'motion/react'
import { Footprints, Flame, Leaf, Users, CalendarHeart, AlertTriangle, Sparkles, Building2 } from 'lucide-react'
import { Logo } from '../components/Logo'
import { ModeToggle } from '../components/ModeToggle'
import { FootstepTrail } from '../components/Footsteps'
import { Card, Pill, ProgressBar } from '../components/ui'
import { useMode } from '../lib/mode'
import { api, type TodayStats, type Reward, type TeamToday } from '../lib/api'

function Ring({ value, children }: { value: number; children: ReactNode }) {
  const r = 52
  const c = 2 * Math.PI * r
  const offset = c - (value / 100) * c
  return (
    <div className="relative grid place-items-center">
      <svg width="132" height="132" viewBox="0 0 132 132" className="-rotate-90">
        <circle cx="66" cy="66" r={r} fill="none" stroke="rgba(15,139,141,0.12)" strokeWidth="12" />
        <motion.circle
          cx="66"
          cy="66"
          r={r}
          fill="none"
          stroke="url(#ringGrad)"
          strokeWidth="12"
          strokeLinecap="round"
          strokeDasharray={c}
          initial={{ strokeDashoffset: c }}
          animate={{ strokeDashoffset: offset }}
          transition={{ duration: 1.1, ease: 'easeOut' }}
        />
        <defs>
          <linearGradient id="ringGrad" x1="0" y1="0" x2="132" y2="132">
            <stop stopColor="#0f8b8d" />
            <stop offset="1" stopColor="#58b86c" />
          </linearGradient>
        </defs>
      </svg>
      <div className="absolute text-center">{children}</div>
    </div>
  )
}

const fade = (i: number) => ({
  initial: { opacity: 0, y: 14 },
  animate: { opacity: 1, y: 0 },
  transition: { duration: 0.45, delay: i * 0.08 },
})

export function Home() {
  const nav = useNavigate()
  const { mode } = useMode()
  const [today, setToday] = useState<TodayStats | null>(null)
  const [team, setTeam] = useState<TeamToday | null>(null)
  const [rewards, setRewards] = useState<Reward[]>([])
  const [teamRewards, setTeamRewards] = useState<Reward[]>([])

  useEffect(() => {
    api.getToday().then(setToday)
    api.getRewards().then(setRewards)
    api.getTeamToday().then(setTeam)
    api.getTeamRewards().then(setTeamRewards)
  }, [])

  const isTeam = mode === 'team'

  return (
    <div className="px-5 pt-5">
      <motion.div {...fade(0)} className="flex items-center justify-between">
        <Logo />
        <ModeToggle />
      </motion.div>

      {/* greeting */}
      <motion.div {...fade(1)} className="mt-5">
        {isTeam ? (
          <>
            <p className="font-display text-[26px] font-semibold leading-tight text-ink">Cześć, {team?.teamName} 🏢</p>
            <p className="text-base text-muted">
              {team?.company} • {team?.members} osób w zespole
            </p>
          </>
        ) : (
          <p className="font-display text-[26px] font-semibold leading-tight text-ink">
            Cześć Ola 👋<br />
            <span className="text-base font-body text-muted">Dobry dzień na spacer nad wodą.</span>
          </p>
        )}
      </motion.div>

      {/* hero stat */}
      <motion.div {...fade(2)} className="mt-4">
        <Card className="relative overflow-hidden p-5">
          <div className="pointer-events-none absolute -right-6 top-2 opacity-40">
            <FootstepTrail count={5} color="#58b86c" />
          </div>
          <div className="flex items-center gap-5">
            <Ring value={(isTeam ? team?.rewardProgress : today?.rewardProgress) ?? 0}>
              <div className="font-display text-2xl font-bold leading-none text-deep">
                {isTeam
                  ? team
                    ? team.steps.toLocaleString('pl-PL')
                    : '—'
                  : today
                    ? today.steps.toLocaleString('pl-PL')
                    : '—'}
              </div>
              <div className="text-[11px] font-bold text-muted">{isTeam ? 'kroków zespołu' : 'kroków dziś'}</div>
            </Ring>
            <div className="flex-1">
              <div className="flex items-baseline gap-1.5">
                <span className="font-display text-4xl font-bold text-sea">
                  {isTeam ? (team?.points ?? '—') : (today?.points ?? '—')}
                </span>
                <span className="text-sm font-bold text-muted">pkt</span>
              </div>
              {isTeam ? (
                <div className="mt-1 flex items-center gap-1.5 text-sm font-bold text-deep">
                  <Building2 size={16} /> wynik zespołu dziś
                </div>
              ) : (
                <div className="mt-1 flex items-center gap-1.5 text-sm font-bold text-[#c8761b]">
                  <Flame size={16} /> {today?.streakDays ?? 0} dni z rzędu
                </div>
              )}
              <div className="mt-3 flex flex-wrap gap-1.5">
                {isTeam ? (
                  <>
                    <Pill tone="sea">
                      <Users size={12} /> spacer grupowy ×{team?.teamMultiplier ?? 2}
                    </Pill>
                    <Pill tone="leaf">
                      <Leaf size={12} /> natura ×3
                    </Pill>
                  </>
                ) : (
                  <>
                    {today?.natureBonusActive && (
                      <Pill tone="leaf">
                        <Leaf size={12} /> natura ×3
                      </Pill>
                    )}
                    <Pill tone={today?.togetherBonusActive ? 'sea' : 'muted'}>
                      <Users size={12} /> z kimś ×1.5
                    </Pill>
                  </>
                )}
              </div>
            </div>
          </div>
        </Card>
      </motion.div>

      {/* progress to reward */}
      {(() => {
        const reward = isTeam ? teamRewards[0] : rewards[0]
        if (!reward) return null
        return (
          <motion.div {...fade(3)} className="mt-4">
            <Card className="p-5">
              <div className="mb-3 flex items-center gap-3">
                <div className="grid h-11 w-11 place-items-center rounded-2xl bg-sand/25 text-2xl">{reward.icon}</div>
                <div className="flex-1">
                  <div className="font-display text-lg font-bold text-ink">{reward.title}</div>
                  <div className="text-xs font-bold text-muted">{reward.kind}</div>
                </div>
                <Pill tone="sand">
                  <Sparkles size={12} /> blisko!
                </Pill>
              </div>
              <ProgressBar value={reward.progress} label={isTeam ? 'Wspólny postęp zespołu' : 'Postęp do nagrody'} />
            </Card>
          </motion.div>
        )
      })()}

      {/* B2B strip (team only) */}
      {isTeam && (
        <motion.div {...fade(4)} className="mt-4">
          <Card className="bg-gradient-to-br from-sea/10 to-leaf/10 p-4">
            <p className="text-sm font-semibold leading-snug text-deep">
              💼 Wellbeing + integracja + eko dla Twojego zespołu — jeden produkt, mierzalny efekt.
            </p>
          </Card>
        </motion.div>
      )}

      {/* quick actions */}
      <motion.div {...fade(5)} className="mt-5">
        <h2 className="mb-3 font-display text-lg font-bold text-ink">Co robimy?</h2>
        <div className="grid grid-cols-3 gap-3">
          <ActionTile onClick={() => nav('/walk')} icon={<Footprints size={22} />} label="Spacer" primary />
          <ActionTile onClick={() => nav('/events')} icon={<CalendarHeart size={22} />} label="Event" />
          <ActionTile onClick={() => nav('/eco')} icon={<AlertTriangle size={22} />} label="Zgłoś" />
        </div>
      </motion.div>

      <motion.p {...fade(6)} className="mt-6 text-center text-xs leading-relaxed text-muted">
        {isTeam ? 'Razem robicie więcej — i dla zespołu, i dla Bałtyku 🌊' : 'Każdy krok liczy się podwójnie, gdy idziesz nad wodą 🌊'}
      </motion.p>
    </div>
  )
}

function ActionTile({
  icon,
  label,
  onClick,
  primary,
}: {
  icon: ReactNode
  label: string
  onClick: () => void
  primary?: boolean
}) {
  return (
    <button
      onClick={onClick}
      className={`flex flex-col items-center gap-2 rounded-3xl px-2 py-4 text-sm font-bold transition active:scale-95 ${
        primary
          ? 'bg-gradient-to-br from-sea to-deep text-white shadow-[0_16px_30px_rgba(12,90,113,0.25)]'
          : 'glass text-deep'
      }`}
    >
      {icon}
      {label}
    </button>
  )
}
