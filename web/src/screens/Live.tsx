import { useEffect, useRef, useState } from 'react'
import { Broadcast, Play, UsersThree, Leaf, Copy, CheckCircle } from '@phosphor-icons/react'
import { ScreenHeader, Card, Pill, PrimaryButton, SoftButton } from '../components/ui'
import { LiveMap, type MapWalker } from '../components/LiveMap'
import { apiRequest, hasBackend, getToken, setToken } from '../lib/http'
import { login, currentUserId, setCurrentUserId } from '../lib/auth'
import { LiveSocket, type ScoredPing, type LeaderRow } from '../lib/ws'

const COLORS = ['#0f8b8d', '#e26d5c', '#7b6cf0', '#f2a541', '#58b86c']

/** Decode the `sub` (user id) claim from a JWT, best-effort. */
function uidFromToken(token: string): string | null {
  try {
    const payload = token.split('.')[1].replace(/-/g, '+').replace(/_/g, '/')
    return JSON.parse(atob(payload)).sub ?? null
  } catch {
    return null
  }
}

interface Walker {
  userId: string
  name: string
  color: string
  trail: { lat: number; lng: number }[]
  points: number
  together: number
  nature: number
  isMe: boolean
}

type Status = 'idle' | 'connecting' | 'live' | 'ended'

interface WalkSession {
  id: string
  join_code: string | null
}

const headerIcon = <Broadcast size={22} weight="fill" />

export function Live() {
  const [authed, setAuthed] = useState<boolean>(!!getToken())
  const [email, setEmail] = useState('ana@demo.walk4change')
  const [pass, setPass] = useState('demodemo')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [sessionId, setSessionId] = useState('')
  const [joinCode, setJoinCode] = useState<string | null>(null)
  const [status, setStatus] = useState<Status>('idle')
  const [copied, setCopied] = useState(false)

  // user_id -> Walker. Kept in a ref for handlers; mirrored to state for render.
  const walkersRef = useRef<Map<string, Walker>>(new Map())
  const [walkers, setWalkers] = useState<Walker[]>([])
  const namesRef = useRef<Map<string, string>>(new Map())
  const [leaderboard, setLeaderboard] = useState<LeaderRow[]>([])
  const socketRef = useRef<LiveSocket | null>(null)

  useEffect(() => () => socketRef.current?.close(), [])

  const nameFor = (userId: string): string => namesRef.current.get(userId) ?? `${userId.slice(0, 4)}…`

  const flush = () => setWalkers(Array.from(walkersRef.current.values()))

  const onPing = (p: ScoredPing) => {
    const map = walkersRef.current
    const prev = map.get(p.user_id)
    const idx = prev ? 0 : map.size
    map.set(p.user_id, {
      userId: p.user_id,
      name: nameFor(p.user_id),
      color: prev?.color ?? COLORS[idx % COLORS.length],
      trail: [...(prev?.trail ?? []), p.point].slice(-40),
      points: parseFloat(p.participant_total),
      together: parseFloat(p.together_mult),
      nature: parseFloat(p.nature_mult),
      isMe: p.user_id === currentUserId(),
    })
    flush()
  }

  const onLeaderboard = (rows: LeaderRow[]) => {
    rows.forEach((r) => namesRef.current.set(r.user_id, r.display_name))
    walkersRef.current.forEach((w, id) => {
      const name = namesRef.current.get(id)
      if (name && name !== w.name) walkersRef.current.set(id, { ...w, name })
    })
    setLeaderboard(rows)
    flush()
  }

  const connect = (id: string) => {
    socketRef.current?.close()
    walkersRef.current = new Map()
    flush()
    setStatus('connecting')
    const sock = new LiveSocket({
      onOpen: () => {
        sock.subscribeSession(id)
        sock.subscribeLeaderboard()
        setStatus('live')
      },
      onPingScored: onPing,
      onLeaderboard,
      onError: (m) => setError(m),
      onClose: () => setStatus((s) => (s === 'live' ? 'ended' : s)),
    })
    socketRef.current = sock
    sock.connect()
  }

  const doLogin = async () => {
    if (busy) return
    setBusy(true)
    setError(null)
    try {
      await login(email, pass)
      setAuthed(true)
    } catch {
      setError('Logowanie nie powiodło się — sprawdź e-mail i hasło.')
    } finally {
      setBusy(false)
    }
  }

  const startWalk = async () => {
    if (busy) return
    setBusy(true)
    setError(null)
    try {
      const res = await apiRequest<WalkSession>('/walks', { method: 'POST' })
      const data = res.data
      if (!data) throw new Error('no session')
      setSessionId(data.id)
      setJoinCode(data.join_code)
      connect(data.id)
    } catch {
      setError('Nie udało się rozpocząć spaceru.')
    } finally {
      setBusy(false)
    }
  }

  const watchExisting = () => {
    if (sessionId.trim()) connect(sessionId.trim())
  }

  const copyCmd = () => {
    navigator.clipboard?.writeText(`./scripts/replay.sh ${sessionId}`).then(
      () => {
        setCopied(true)
        window.setTimeout(() => setCopied(false), 1500)
      },
      () => {},
    )
  }

  // Deep link: /live?token=<jwt>&watch=<session-id> auto-logs-in and auto-watches.
  // Used by `make demo` so the whole demo is one click. Token is stripped from the
  // address bar after it's read.
  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const t = params.get('token')
    const watch = params.get('watch') ?? params.get('session')
    if (!t && !watch) return
    if (t) {
      setToken(t)
      const uid = uidFromToken(t)
      if (uid) setCurrentUserId(uid)
      setAuthed(true)
    }
    if (watch) {
      setSessionId(watch)
      connect(watch)
    }
    window.history.replaceState(null, '', window.location.pathname)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // ── No backend configured ───────────────────────────────────────────────────
  if (!hasBackend()) {
    return (
      <div className="px-4 py-6">
        <ScreenHeader icon={headerIcon} title="Na żywo" subtitle="Spacer dwojga osób w czasie rzeczywistym" />
        <Card className="mt-4 p-4">
          <p className="text-sm text-muted">
            Ten ekran wymaga backendu. Ustaw <code>VITE_API_BASE</code> (np. <code>http://localhost:8080</code>) w{' '}
            <code>web/.env.local</code> i odśwież.
          </p>
        </Card>
      </div>
    )
  }

  // ── Needs login (for the WS JWT) ────────────────────────────────────────────
  if (!authed) {
    return (
      <div className="px-4 py-6">
        <ScreenHeader icon={headerIcon} title="Na żywo" subtitle="Zaloguj się, aby śledzić spacer na żywo" />
        <Card className="mt-4 p-4">
          <label className="block text-xs font-bold uppercase tracking-wide text-muted">E-mail</label>
          <input value={email} onChange={(e) => setEmail(e.target.value)} type="email" className="mt-1 w-full rounded-xl border border-white/70 bg-white/80 px-3 py-2 text-sm text-ink outline-none" />
          <label className="mt-3 block text-xs font-bold uppercase tracking-wide text-muted">Hasło</label>
          <input value={pass} onChange={(e) => setPass(e.target.value)} type="password" className="mt-1 w-full rounded-xl border border-white/70 bg-white/80 px-3 py-2 text-sm text-ink outline-none" />
          {error && <p className="mt-3 text-sm font-semibold text-rose-600">{error}</p>}
          <PrimaryButton onClick={doLogin} className="mt-4 w-full">
            {busy ? 'Logowanie…' : 'Zaloguj się (demo: Ana)'}
          </PrimaryButton>
        </Card>
      </div>
    )
  }

  // Combined multiplier across active walkers (best together × nature).
  const combined = walkers.reduce((m, w) => Math.max(m, w.together * w.nature), 0)

  return (
    <div className="px-4 py-6">
      <ScreenHeader icon={headerIcon} title="Na żywo" subtitle="Spacer dwojga osób w czasie rzeczywistym" />

      {status === 'idle' ? (
        <Card className="mt-4 p-4">
          <p className="text-sm text-muted">
            Rozpocznij spacer, a potem w terminalu odpal symulację drugiej osoby:
            <br />
            <code>./scripts/replay.sh &lt;id-sesji&gt;</code>
          </p>
          <PrimaryButton onClick={startWalk} className="mt-4 w-full">
            <Play size={18} weight="fill" /> {busy ? 'Chwila…' : 'Rozpocznij spacer'}
          </PrimaryButton>
          <div className="mt-4">
            <label className="block text-xs font-bold uppercase tracking-wide text-muted">…albo podglądaj istniejącą sesję</label>
            <div className="mt-1 flex gap-2">
              <input value={sessionId} onChange={(e) => setSessionId(e.target.value)} placeholder="session-id" className="w-full rounded-xl border border-white/70 bg-white/80 px-3 py-2 text-sm text-ink outline-none" />
              <SoftButton onClick={watchExisting}>Podglądaj</SoftButton>
            </div>
          </div>
          {error && <p className="mt-3 text-sm font-semibold text-rose-600">{error}</p>}
        </Card>
      ) : (
        <>
          <Card className="mt-4 p-4">
            <div className="flex items-center justify-between">
              <span className="inline-flex items-center gap-2 text-sm font-bold text-deep">
                <span className={`h-2.5 w-2.5 rounded-full ${status === 'live' ? 'animate-pulse bg-leaf' : 'bg-muted'}`} />
                {status === 'live' ? 'NA ŻYWO' : status === 'connecting' ? 'łączę…' : 'zakończono'}
              </span>
              <div className="flex items-center gap-1.5">
                <Pill tone="leaf"><UsersThree size={14} weight="fill" /> razem ×{walkers[0]?.together ?? 1}</Pill>
                <Pill tone="sea"><Leaf size={14} weight="fill" /> natura ×{walkers[0]?.nature ?? 1}</Pill>
              </div>
            </div>
            {combined > 1 && (
              <p className="mt-2 text-center text-2xl font-extrabold text-deep">
                ×{combined.toFixed(1)} <span className="text-sm font-semibold text-muted">mnożnik punktów</span>
              </p>
            )}
          </Card>

          <Card className="mt-3 p-2">
            <LiveMap walkers={walkers as MapWalker[]} />
          </Card>

          <div className="mt-3 grid grid-cols-2 gap-2">
            {walkers.map((w) => (
              <Card key={w.userId} className="p-3">
                <div className="flex items-center gap-2">
                  <span className="h-3 w-3 rounded-full" style={{ background: w.color }} />
                  <span className="text-sm font-bold text-ink">{w.name}{w.isMe ? ' (Ty)' : ''}</span>
                </div>
                <p className="mt-1 text-xl font-extrabold text-deep">{w.points.toFixed(1)} <span className="text-xs font-semibold text-muted">pkt</span></p>
                <p className="text-[11px] text-muted">razem ×{w.together} · natura ×{w.nature}</p>
              </Card>
            ))}
          </div>

          {joinCode && (
            <Card className="mt-3 p-3">
              <p className="text-xs text-muted">Symuluj drugą osobę (Bek) w tej sesji:</p>
              <div className="mt-1 flex items-center justify-between gap-2">
                <code className="truncate text-xs text-ink">./scripts/replay.sh {sessionId}</code>
                <button onClick={copyCmd} className="shrink-0 text-sea" aria-label="Kopiuj komendę">
                  {copied ? <CheckCircle size={18} weight="fill" /> : <Copy size={18} />}
                </button>
              </div>
            </Card>
          )}

          {leaderboard.length > 0 && (
            <Card className="mt-3 p-4">
              <p className="text-xs font-bold uppercase tracking-wide text-muted">Ranking (na żywo)</p>
              <ul className="mt-2 space-y-1">
                {leaderboard.slice(0, 5).map((r, i) => (
                  <li key={r.user_id} className="flex items-center justify-between text-sm">
                    <span className="text-ink">{i + 1}. {r.display_name}</span>
                    <span className="font-bold text-deep">{Math.round(parseFloat(r.total_points))}</span>
                  </li>
                ))}
              </ul>
            </Card>
          )}
        </>
      )}
    </div>
  )
}
