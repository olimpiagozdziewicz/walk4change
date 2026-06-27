import { useEffect, useState } from 'react'
import { motion } from 'motion/react'
import { MapPin, Clock, Trophy, Users } from 'lucide-react'
import { ScreenHeader, Card, Pill, PrimaryButton, SoftButton } from '../components/ui'
import { useMode } from '../lib/mode'
import { api, type CommunityWalk, type LeaderboardRow, type TeamRow } from '../lib/api'

export function Community() {
  const { mode } = useMode()
  const isTeam = mode === 'team'
  const [walks, setWalks] = useState<CommunityWalk[]>([])
  const [board, setBoard] = useState<LeaderboardRow[]>([])
  const [teamBoard, setTeamBoard] = useState<TeamRow[]>([])
  const [joined, setJoined] = useState<Record<string, boolean>>({})

  useEffect(() => {
    api.getCommunityWalks().then(setWalks)
    api.getLeaderboard().then(setBoard)
    api.getTeamLeaderboard().then(setTeamBoard)
  }, [])

  return (
    <div>
      <ScreenHeader
        title={isTeam ? 'Zespół' : 'Społeczność'}
        emoji={isTeam ? '🏢' : '🤝'}
        subtitle={isTeam ? 'Wspólne spacery działów i ranking zespołów w firmie.' : 'Nikt nie musi chodzić sam. Dołącz albo zaproś kogoś.'}
      />

      <div className="space-y-3 px-5 pt-2">
        {walks.map((w, i) => (
          <motion.div key={w.id} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.08 }}>
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

        {/* leaderboard */}
        <div className="pt-3">
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
                      <Users size={16} />
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
        </div>
      </div>
    </div>
  )
}
