import { useEffect, useRef, useState } from 'react'
import { motion, AnimatePresence } from 'motion/react'
import { Play, Square, UsersThree, Leaf, Trophy, Footprints, MapPin, SignIn, Copy, CheckCircle, HandHeart } from '@phosphor-icons/react'
import { ScreenHeader, Card, PrimaryButton, SoftButton, Pill } from '../components/ui'
import { FootstepTrail } from '../components/Footsteps'
import { Celebrate } from '../components/Celebrate'
import { LiveMap, type MapWalker } from '../components/LiveMap'
import { apiRequest, hasBackend, getToken } from '../lib/http'
import { login, register, currentUserId, requestMagicLink } from '../lib/auth'
import { LiveSocket, type ScoredPing, type LeaderRow } from '../lib/ws'
import { useStepCounter } from '../hooks/useStepCounter'

const COLORS = ['#0f8b8d', '#e26d5c', '#7b6cf0', '#f2a541', '#58b86c']

type Phase = 'auth' | 'idle' | 'active' | 'summary'

interface Walker {
  userId: string
  name: string
  color: string
  trail: { lat: number; lng: number }[]
  points: number
  meters: number
  together: number
  nature: number
  isMe: boolean
}

interface WalkSession {
  id: string
  join_code: string | null
}

function fmt(sec: number) {
  const m = Math.floor(sec / 60)
  const s = sec % 60
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
}

export function Walk() {
  const [phase, setPhase] = useState<Phase>(getToken() ? 'idle' : 'auth')
  const [sec, setSec] = useState(0)
  const timer = useRef<number | null>(null)

  // auth
  const [mode, setMode] = useState<'login' | 'signup'>('login')
  const [email, setEmail] = useState('')
  const [pass, setPass] = useState('')
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [magicMsg, setMagicMsg] = useState<string | null>(null)

  // walk + pairing
  const [sessionId, setSessionId] = useState('')
  const [joinCode, setJoinCode] = useState<string | null>(null)
  const [codeInput, setCodeInput] = useState('')
  const [copied, setCopied] = useState(false)
  const [gpsNote, setGpsNote] = useState<string | null>(null)

  // live state
  const walkersRef = useRef<Map<string, Walker>>(new Map())
  const [walkers, setWalkers] = useState<Walker[]>([])
  const namesRef = useRef<Map<string, string>>(new Map())
  const [leaderboard, setLeaderboard] = useState<LeaderRow[]>([])
  const socketRef = useRef<LiveSocket | null>(null)
  const seqRef = useRef(0)
  const watchRef = useRef<number | null>(null)
  const lastSentRef = useRef(0)
  const [summary, setSummary] = useState<{ points: number; meters: number; steps: number; together: boolean; nature: boolean } | null>(null)
  const { steps, source: stepSource, permissionNeeded, requestPermission, addMeters, reset: resetSteps } = useStepCounter()

  useEffect(() => {
    if (phase === 'active') {
      timer.current = window.setInterval(() => setSec((s) => s + 1), 1000)
    }
    return () => { if (timer.current) window.clearInterval(timer.current) }
  }, [phase])

  useEffect(() => () => stopStreaming(), [])

  const flush = () => setWalkers(Array.from(walkersRef.current.values()))
  const nameFor = (id: string) => namesRef.current.get(id) ?? `${id.slice(0, 4)}…`
  const me = () => walkers.find((w) => w.isMe)

  const onPing = (p: ScoredPing) => {
    const map = walkersRef.current
    const prev = map.get(p.user_id)
    const idx = prev ? 0 : map.size
    const seg = parseFloat(p.segment_meters)
    map.set(p.user_id, {
      userId: p.user_id,
      name: nameFor(p.user_id),
      color: prev?.color ?? COLORS[idx % COLORS.length],
      trail: [...(prev?.trail ?? []), p.point].slice(-50),
      points: parseFloat(p.participant_total),
      meters: (prev?.meters ?? 0) + seg,
      together: parseFloat(p.together_mult),
      nature: parseFloat(p.nature_mult),
      isMe: p.user_id === currentUserId(),
    })
    if (p.user_id === currentUserId()) addMeters(seg)
    flush()
  }

  const onLeaderboard = (rows: LeaderRow[]) => {
    rows.forEach((r) => namesRef.current.set(r.user_id, r.display_name))
    walkersRef.current.forEach((w, id) => {
      const nm = namesRef.current.get(id)
      if (nm && nm !== w.name) walkersRef.current.set(id, { ...w, name: nm })
    })
    setLeaderboard(rows)
    flush()
  }

  const doAuth = async () => {
    if (busy) return
    setBusy(true); setError(null)
    try {
      if (mode === 'signup') await register(email.trim(), pass, name.trim() || email.split('@')[0])
      else await login(email.trim(), pass)
      setPhase('idle')
    } catch {
      setError(mode === 'signup' ? 'Rejestracja nieudana (hasło min. 8 znaków, e-mail z @).' : 'Logowanie nieudane — sprawdź dane.')
    } finally { setBusy(false) }
  }

  const doMagic = async () => {
    if (busy) return
    if (!email.includes('@')) { setError('Podaj e-mail, aby wysłać magiczny link.'); return }
    setBusy(true); setError(null); setMagicMsg(null)
    try {
      await requestMagicLink(email)
      setMagicMsg(`✓ Sprawdź skrzynkę — magiczny link poszedł na ${email.trim()}.`)
    } catch {
      setError('Nie udało się wysłać magicznego linku.')
    } finally { setBusy(false) }
  }

  const connectAndStream = (id: string) => {
    socketRef.current?.close()
    walkersRef.current = new Map(); flush()
    seqRef.current = 0; setSec(0); setSummary(null); resetSteps()
    const sock = new LiveSocket({
      onOpen: () => { sock.subscribeSession(id); sock.subscribeLeaderboard() },
      onPingScored: onPing,
      onLeaderboard,
      onError: (m) => setError(m),
    })
    socketRef.current = sock
    sock.connect()
    startGps(id)
    setPhase('active')
  }

  const startWalk = async () => {
    if (busy) return
    setBusy(true); setError(null)
    try {
      const res = await apiRequest<WalkSession>('/walks', { method: 'POST' })
      if (!res.data) throw new Error('no session')
      setSessionId(res.data.id); setJoinCode(res.data.join_code); setCodeInput('')
      connectAndStream(res.data.id)
    } catch { setError('Nie udało się rozpocząć spaceru.') } finally { setBusy(false) }
  }

  const joinWalk = async () => {
    if (busy) return
    const code = codeInput.trim().toUpperCase()
    if (!code) return
    setBusy(true); setError(null)
    try {
      const res = await apiRequest<{ session_id: string }>('/walks/join-by-code', { method: 'POST', body: { code } })
      if (!res.data?.session_id) throw new Error('bad code')
      setSessionId(res.data.session_id); setJoinCode(code)
      connectAndStream(res.data.session_id)
    } catch { setError('Nie znaleziono spaceru o tym kodzie (musi być aktywny).') } finally { setBusy(false) }
  }

  // ── automatic GPS: find position + stream it; modifiers come back from server ──
  const startGps = (id: string) => {
    if (!('geolocation' in navigator)) { setGpsNote('Brak GPS w tej przeglądarce.'); return }
    setGpsNote('Szukam pozycji GPS… (zezwól na lokalizację)')
    watchRef.current = navigator.geolocation.watchPosition(
      (pos) => {
        setGpsNote(null)
        const now = Date.now()
        if (now - lastSentRef.current < 1200) return
        lastSentRef.current = now
        seqRef.current += 1
        socketRef.current?.sendPing(id, seqRef.current, pos.coords.latitude, pos.coords.longitude)
      },
      (err) => setGpsNote(`GPS niedostępny: ${err.message}. Włącz lokalizację i odśwież.`),
      { enableHighAccuracy: true, maximumAge: 1000, timeout: 15000 },
    )
  }

  const stopStreaming = () => {
    if (watchRef.current != null) { navigator.geolocation.clearWatch(watchRef.current); watchRef.current = null }
    socketRef.current?.close(); socketRef.current = null
    if (timer.current) window.clearInterval(timer.current)
  }

  const stopWalk = async () => {
    const mine = me()
    setSummary({ points: mine?.points ?? 0, meters: mine?.meters ?? 0, steps, together: (mine?.together ?? 1) > 1, nature: (mine?.nature ?? 1) > 1 })
    stopStreaming()
    try { await apiRequest(`/walks/${sessionId}/stop`, { method: 'POST' }) } catch { /* non-host */ }
    try { await apiRequest(`/walks/${sessionId}/leave`, { method: 'POST' }) } catch { /* ignore */ }
    setPhase('summary')
  }

  const copyCode = () => {
    if (!joinCode) return
    navigator.clipboard?.writeText(joinCode).then(() => { setCopied(true); window.setTimeout(() => setCopied(false), 1500) }, () => {})
  }

  // ── render ──────────────────────────────────────────────────────────────────
  if (!hasBackend()) {
    return <div><ScreenHeader title="Spacer" icon={<Footprints size={22} />} /><div className="px-5"><Card className="p-4"><p className="text-sm text-muted">Ten ekran wymaga backendu (VITE_API_BASE).</p></Card></div></div>
  }

  const mine = me()
  const combined = walkers.reduce((m, w) => Math.max(m, w.together * w.nature), 0)

  return (
    <div>
      <ScreenHeader title="Spacer" icon={<Footprints size={22} />} subtitle="Każdy krok to punkty. We dwoje i na łonie natury — jeszcze więcej." />
      <div className="px-5">
        <AnimatePresence mode="wait">
          {/* ── AUTH ── */}
          {phase === 'auth' && (
            <motion.div key="auth" initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
              <Card className="mt-2 p-5">
                <div className="mb-3 flex gap-2">
                  <button onClick={() => setMode('login')} className={`flex-1 rounded-xl py-2 text-sm font-bold ${mode === 'login' ? 'bg-sea/15 text-deep' : 'text-muted'}`}>Logowanie</button>
                  <button onClick={() => setMode('signup')} className={`flex-1 rounded-xl py-2 text-sm font-bold ${mode === 'signup' ? 'bg-sea/15 text-deep' : 'text-muted'}`}>Rejestracja</button>
                </div>
                {mode === 'signup' && (<><label className="block text-xs font-bold uppercase tracking-wide text-muted">Imię</label><input value={name} onChange={(e) => setName(e.target.value)} className="mb-3 mt-1 w-full rounded-xl border border-white/70 bg-white/80 px-3 py-2 text-sm outline-none" placeholder="Ola" /></>)}
                <label className="block text-xs font-bold uppercase tracking-wide text-muted">E-mail</label>
                <input value={email} onChange={(e) => setEmail(e.target.value)} type="email" autoCapitalize="none" className="mt-1 w-full rounded-xl border border-white/70 bg-white/80 px-3 py-2 text-sm outline-none" placeholder="ty@email.pl" />
                <label className="mt-3 block text-xs font-bold uppercase tracking-wide text-muted">Hasło</label>
                <input value={pass} onChange={(e) => setPass(e.target.value)} type="password" className="mt-1 w-full rounded-xl border border-white/70 bg-white/80 px-3 py-2 text-sm outline-none" placeholder="min. 8 znaków" />
                {error && <p className="mt-3 text-sm font-semibold text-rose-600">{error}</p>}
                <PrimaryButton onClick={doAuth} className="mt-4 w-full"><SignIn size={18} /> {busy ? 'Chwila…' : mode === 'signup' ? 'Załóż konto' : 'Zaloguj się'}</PrimaryButton>
                <button onClick={doMagic} disabled={busy} className="mt-3 w-full text-center text-sm font-bold text-sea disabled:opacity-60">albo wyślij magiczny link →</button>
                {magicMsg && <p className="mt-2 text-sm font-semibold text-[#2f7a45]">{magicMsg}</p>}
              </Card>
            </motion.div>
          )}

          {/* ── IDLE ── */}
          {phase === 'idle' && (
            <motion.div key="idle" initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
              <Card className="relative mt-2 overflow-hidden p-6 text-center">
                <div className="pointer-events-none absolute inset-x-0 bottom-2 flex justify-center opacity-50"><FootstepTrail count={6} color="#0f8b8d" /></div>
                <div className="relative">
                  <div className="mx-auto mb-4 grid h-20 w-20 place-items-center rounded-full bg-gradient-to-br from-sea/15 to-leaf/15 text-sea"><Footprints size={40} weight="fill" /></div>
                  <h2 className="font-display text-2xl font-bold text-ink">Gotowi na spacer?</h2>
                  <p className="mx-auto mt-2 max-w-[280px] text-sm text-muted">SeaSteps sam wykryje Twoją pozycję GPS i naliczy bonusy: <b className="text-deep">we dwoje ×1.5</b>, <b className="text-deep">natura ×3</b>.</p>
                </div>
              </Card>
              <PrimaryButton onClick={startWalk} className="mt-5 w-full py-4 text-base"><Play size={20} weight="fill" color="white" /> {busy ? 'Chwila…' : 'Rozpocznij spacer'}</PrimaryButton>
              <Card className="mt-3 p-4">
                <label className="block text-xs font-bold uppercase tracking-wide text-muted">…albo dołącz do znajomego — wpisz jego kod</label>
                <div className="mt-1 flex gap-2">
                  <input value={codeInput} onChange={(e) => setCodeInput(e.target.value.toUpperCase())} placeholder="np. 4G3YL7OA" maxLength={8} className="w-full rounded-xl border border-white/70 bg-white/80 px-3 py-2 font-mono text-sm tracking-widest outline-none" />
                  <SoftButton onClick={joinWalk}>Dołącz</SoftButton>
                </div>
                {error && <p className="mt-3 text-sm font-semibold text-rose-600">{error}</p>}
              </Card>
            </motion.div>
          )}

          {/* ── ACTIVE ── */}
          {phase === 'active' && (
            <motion.div key="active" initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
              <Card className="relative mt-2 overflow-hidden p-6">
                <div className="pointer-events-none absolute right-3 top-0 opacity-70"><FootstepTrail count={7} color="#58b86c" /></div>
                <div className="text-center">
                  <Pill tone="leaf">● na żywo</Pill>
                  <div className="mt-3 font-display text-6xl font-bold tabular-nums tracking-tight text-deep">{fmt(sec)}</div>
                  <div className="mt-1 text-sm font-bold text-muted">czas spaceru</div>
                </div>
                <div className="mt-6 grid grid-cols-3 gap-3">
                  <Stat label="kroki" value={steps.toLocaleString('pl-PL')} accent />
                  <Stat label="metry" value={Math.round(mine?.meters ?? 0).toLocaleString('pl-PL')} />
                  <Stat label="punkty" value={(mine?.points ?? 0).toFixed(1)} accent />
                </div>
                <div className="mt-3 flex flex-wrap justify-center gap-2">
                  <Pill tone="leaf"><Leaf size={12} /> natura ×{mine?.nature ?? 1}</Pill>
                  <Pill tone="sea"><UsersThree size={12} /> we dwoje ×{mine?.together ?? 1}</Pill>
                  {combined > 1 && <Pill tone="sand">razem ×{combined.toFixed(1)}</Pill>}
                  <Pill tone="muted"><Footprints size={12} /> {stepSource === 'accelerometer' ? 'sensor' : 'GPS'}</Pill>
                </div>
                {permissionNeeded && (
                  <button onClick={requestPermission} className="mt-3 w-full rounded-2xl bg-sea/10 py-2 text-xs font-bold text-sea">
                    Zezwól na ruch, by liczyć kroki dokładniej →
                  </button>
                )}
                {gpsNote && <p className="mt-3 inline-flex items-center gap-1 text-xs font-semibold text-amber-600"><MapPin size={14} weight="fill" /> {gpsNote}</p>}
                {error && <p className="mt-2 text-xs font-semibold text-rose-600">{error}</p>}
              </Card>

              {joinCode && (
                <Card className="mt-3 p-3">
                  <p className="text-xs text-muted">Kod dla osoby obok (wpisuje go w „Dołącz"):</p>
                  <div className="mt-1 flex items-center justify-between">
                    <code className="text-2xl font-extrabold tracking-widest text-deep">{joinCode}</code>
                    <button onClick={copyCode} className="text-sea" aria-label="Kopiuj kod">{copied ? <CheckCircle size={22} weight="fill" /> : <Copy size={22} />}</button>
                  </div>
                </Card>
              )}

              {walkers.some((w) => w.trail.length > 0) && <Card className="mt-3 p-2"><LiveMap walkers={walkers as MapWalker[]} /></Card>}

              {walkers.length > 1 && (
                <div className="mt-3 grid grid-cols-2 gap-2">
                  {walkers.map((w) => (
                    <Card key={w.userId} className="p-3">
                      <div className="flex items-center gap-2"><span className="h-3 w-3 rounded-full" style={{ background: w.color }} /><span className="text-sm font-bold text-ink">{w.name}{w.isMe ? ' (Ty)' : ''}</span></div>
                      <p className="mt-1 text-lg font-extrabold text-deep">{w.points.toFixed(1)} <span className="text-xs font-semibold text-muted">pkt</span></p>
                    </Card>
                  ))}
                </div>
              )}

              <button onClick={stopWalk} className="mt-5 inline-flex w-full items-center justify-center gap-2 rounded-2xl border border-[#e6b4b4] bg-white/80 py-4 text-base font-bold text-[#c0504d] transition active:scale-[0.97]"><Square size={18} weight="fill" color="#c0504d" /> Zakończ spacer</button>
            </motion.div>
          )}

          {/* ── SUMMARY ── */}
          {phase === 'summary' && summary && (
            <motion.div key="summary" initial={{ opacity: 0, scale: 0.96 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0 }}>
              <Celebrate />
              <Card className="mt-2 overflow-hidden p-6 text-center">
                <motion.div initial={{ scale: 0, rotate: -20 }} animate={{ scale: 1, rotate: 0 }} transition={{ type: 'spring', delay: 0.1 }} className="mx-auto mb-3 grid h-20 w-20 place-items-center rounded-full bg-gradient-to-br from-sea to-leaf shadow-[0_16px_30px_rgba(15,139,141,0.3)]"><Trophy size={36} color="white" /></motion.div>
                <h2 className="font-display text-2xl font-bold text-ink">Brawo, spacer zaliczony!</h2>
                <p className="mt-1 text-sm text-muted">{fmt(sec)} • {summary.steps > 0 ? `${summary.steps.toLocaleString('pl-PL')} kroków • ` : ''}{Math.round(summary.meters).toLocaleString('pl-PL')} m</p>
                <div className="mt-5 rounded-3xl bg-gradient-to-br from-sea/10 to-leaf/10 p-5">
                  <div className="font-display text-5xl font-bold text-sea">+{summary.points.toFixed(1)}</div>
                  <div className="text-sm font-bold text-muted">punktów zdobytych</div>
                  <div className="mt-2 flex justify-center gap-2">
                    {summary.nature && <Pill tone="leaf"><Leaf size={12} /> natura ×3</Pill>}
                    {summary.together && <Pill tone="sea"><UsersThree size={12} /> we dwoje ×1.5</Pill>}
                  </div>
                </div>
                <p className="mt-4 inline-flex items-center justify-center gap-1.5 text-sm font-bold text-[#2f7a45]"><HandHeart size={16} weight="fill" /> Jesteś coraz bliżej adopcji foki!</p>
              </Card>
              {leaderboard.length > 0 && (
                <Card className="mt-4 p-4">
                  <p className="text-xs font-bold uppercase tracking-wide text-muted">Ranking</p>
                  <ul className="mt-2 space-y-1">{leaderboard.slice(0, 5).map((r, i) => (<li key={r.user_id} className="flex items-center justify-between text-sm"><span className="text-ink">{i + 1}. {r.display_name}</span><span className="font-bold text-deep">{Math.round(parseFloat(r.total_points))}</span></li>))}</ul>
                </Card>
              )}
              <PrimaryButton onClick={() => setPhase('idle')} className="mt-4 w-full">Nowy spacer</PrimaryButton>
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
