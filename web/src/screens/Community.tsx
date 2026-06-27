import { useEffect, useState } from 'react'
import { motion } from 'motion/react'
import { MapPin, Clock, Trophy, UsersThree, Footprints, ChatCircle } from '@phosphor-icons/react'
import { ScreenHeader, Card, Pill, PrimaryButton, SoftButton } from '../components/ui'
import { useMode } from '../lib/mode'
import { getInterests } from '../lib/interests'
import { api, type CommunityWalk, type LeaderboardRow, type TeamRow, type MatchPerson } from '../lib/api'

export function Community() {
  const { mode } = useMode()
  const isTeam = mode === 'team'
  const [walks, setWalks] = useState<CommunityWalk[]>([])
  const [board, setBoard] = useState<LeaderboardRow[]>([])
  const [teamBoard, setTeamBoard] = useState<TeamRow[]>([])
  const [matches, setMatches] = useState<MatchPerson[]>([])
  const [joined, setJoined] = useState<Record<string, boolean>>({})
  const [invited, setInvited] = useState<Record<string, 'walk' | 'chat' | undefined>>({})
  const myInterests = getInterests()

  useEffect(() => {
    api.getCommunityWalks().then(setWalks)
    api.getLeaderboard().then(setBoard)
    api.getTeamLeaderboard().then(setTeamBoard)
    api.getMatches().then(setMatches)
  }, [])

  return (
    <div>
      <ScreenHeader
        title={isTeam ? 'Zespół' : 'Społeczność'}
        emoji={isTeam ? '🏢' : '🤝'}
        subtitle={isTeam ? 'Wspólne spacery działów i ranking zespołów.' : 'Dopasuj się do ludzi, umów się na spacer albo rozmowę.'}
      />

      <div className="space-y-4 px-5 pt-2">
        {/* ── Dopasowani do Ciebie (tylko solo) ── */}
        {!isTeam && (
          <section>
            <h2 className="mb-3 flex items-center gap-2 font-display text-lg font-bold text-ink">
              💚 Dopasowani do Ciebie
            </h2>
            <div className="space-y-3">
              {matches.map((m, i) => {
                const shared = m.interests.filter((t) => myInterests.includes(t))
                const inv = invited[m.id]
                return (
                  <motion.div key={m.id} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.07 }}>
                    <Card className="p-4">
                      <div className="flex items-center gap-3">
                        <div className="grid h-12 w-12 place-items-center rounded-2xl bg-gradient-to-br from-sea/12 to-leaf/12 text-2xl">
                          {m.avatar}
                        </div>
                        <div className="flex-1">
                          <div className="flex items-center gap-2">
                            <span className="font-display text-lg font-bold text-ink">{m.name}</span>
                            <span className="inline-flex items-center gap-0.5 text-xs font-bold text-muted">
                              <MapPin size={12} /> {m.distance}
                            </span>
                          </div>
                          <p className="text-xs text-muted">{m.bio}</p>
                        </div>
                      </div>

                      {shared.length > 0 && (
                        <div className="mt-3 flex flex-wrap gap-1.5">
                          <span className="text-xs font-bold text-leaf">Wspólne:</span>
                          {shared.map((s) => (
                            <Pill key={s} tone="leaf">
                              {s}
                            </Pill>
                          ))}
                        </div>
                      )}

                      <div className="mt-3 grid grid-cols-2 gap-2.5">
                        <SoftButton
                          onClick={() => setInvited((v) => ({ ...v, [m.id]: inv === 'chat' ? undefined : 'chat' }))}
                          className="py-2.5 text-sm"
                        >
                          <ChatCircle size={16} /> {inv === 'chat' ? 'Wysłano' : 'Rozmowa'}
                        </SoftButton>
                        <PrimaryButton
                          onClick={() => setInvited((v) => ({ ...v, [m.id]: inv === 'walk' ? undefined : 'walk' }))}
                          className="py-2.5 text-sm"
                        >
                          <Footprints size={16} /> {inv === 'walk' ? '✓ Zaproszona' : 'Umów spacer'}
                        </PrimaryButton>
                      </div>
                    </Card>
                  </motion.div>
                )
              })}
            </div>
          </section>
        )}

        {/* ── Spacery / wspólne wyjścia ── */}
        <section>
          <h2 className="mb-3 font-display text-lg font-bold text-ink">{isTeam ? 'Spacery działów' : 'Wspólne spacery'}</h2>
          <div className="space-y-3">
            {walks.map((w, i) => (
              <motion.div key={w.id} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.06 }}>
                <Card className="p-4">
                  <div className="flex items-center gap-3">
                    <div className="grid h-12 w-12 place-items-center rounded-2xl bg-sea/10 text-2xl">{w.avatar}</div>
                    <div className="flex-1">
                      <div className="font-display text-lg font-bold text-ink">{w.who}</div>
                      <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs font-bold text-muted">
                        <span className="inline-flex items-center gap-1">
                          <MapPin size={12} /> {w.where}
                        </span>
                        <span className="inline-flex items-center gap-1">
                          <Clock size={12} /> {w.when}
                        </span>
                      </div>
                    </div>
                  </div>
                  <p className="mt-2.5 text-sm text-muted">„{w.vibe}"</p>
                  <div className="mt-3 grid grid-cols-2 gap-2.5">
                    <SoftButton onClick={() => {}} className="py-2.5 text-sm">
                      Zaproś
                    </SoftButton>
                    <PrimaryButton onClick={() => setJoined((j) => ({ ...j, [w.id]: !j[w.id] }))} className="py-2.5 text-sm">
                      {joined[w.id] ? '✓ Idę!' : 'Dołączam'}
                    </PrimaryButton>
                  </div>
                </Card>
              </motion.div>
            ))}
          </div>
        </section>

        {/* ── Ranking ── */}
        <section>
          <h2 className="mb-3 flex items-center gap-2 font-display text-lg font-bold text-ink">
            <Trophy size={18} className="text-sand" /> {isTeam ? 'Ranking zespołów' : 'Ranking tygodnia'}
          </h2>
          <Card className="divide-y divide-[rgba(20,52,58,0.06)] p-2">
            {isTeam
              ? teamBoard.map((r) => (
                  <div key={r.rank} className={`flex items-center gap-3 rounded-2xl px-3 py-2.5 ${r.isMine ? 'bg-sea/8' : ''}`}>
                    <span className={`w-6 text-center font-display text-lg font-bold ${r.rank <= 3 ? 'text-sea' : 'text-muted'}`}>
                      {r.rank}
                    </span>
                    <span className="grid h-9 w-9 place-items-center rounded-full bg-white/70 text-sea">
                      <UsersThree size={16} />
                    </span>
                    <span className={`flex-1 text-sm font-bold ${r.isMine ? 'text-deep' : 'text-ink'}`}>
                      {r.team} {r.isMine && <Pill tone="sea">Twój dział</Pill>}
                      <span className="ml-1 text-xs font-semibold text-muted">• {r.members} os.</span>
                    </span>
                    <span className="font-display text-base font-bold text-sea">{r.points}</span>
                  </div>
                ))
              : board.map((r) => (
                  <div key={r.rank} className={`flex items-center gap-3 rounded-2xl px-3 py-2.5 ${r.isMe ? 'bg-sea/8' : ''}`}>
                    <span className={`w-6 text-center font-display text-lg font-bold ${r.rank <= 3 ? 'text-sea' : 'text-muted'}`}>
                      {r.rank}
                    </span>
                    <span className="grid h-9 w-9 place-items-center rounded-full bg-white/70 text-lg">{r.avatar}</span>
                    <span className={`flex-1 text-sm font-bold ${r.isMe ? 'text-deep' : 'text-ink'}`}>
                      {r.name} {r.isMe && <Pill tone="sea">to Ty</Pill>}
                    </span>
                    <span className="font-display text-base font-bold text-sea">{r.points}</span>
                  </div>
                ))}
          </Card>
        </section>
      </div>
    </div>
  )
}
