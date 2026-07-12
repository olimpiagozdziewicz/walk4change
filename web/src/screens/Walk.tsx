import { useEffect, useRef, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { motion, AnimatePresence } from 'motion/react'
import { Play, Square, UsersThree, Leaf, Trophy, Footprints, MapPin, SignIn, Copy, CheckCircle, HandHeart, ThumbsUp, ThumbsDown } from '@phosphor-icons/react'
import { ScreenHeader, Card, PrimaryButton, SoftButton, Pill } from '../components/ui'
import { FootstepTrail } from '../components/Footsteps'
import { Celebrate } from '../components/Celebrate'
import { RealMap } from '../components/RealMap'
import { apiRequest, hasBackend, getToken } from '../lib/http'
import { login, register, currentUserId, requestMagicLink } from '../lib/auth'
import { LiveSocket, type ScoredPing, type LeaderRow } from '../lib/ws'
import { useStepCounter } from '../hooks/useStepCounter'
import { addWalk } from '../lib/walks'
import { api, type WalkDetailInfo, type RatingFlag } from '../lib/api'

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
  const [searchParams, setSearchParams] = useSearchParams()
  const [phase, setPhase] = useState<Phase>(getToken() ? 'idle' : 'auth')
  const [sec, setSec] = useState(0)
  const timer = useRef<number | null>(null)
  const joinedViaQueryRef = useRef(false)

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

  // "spaceruję — dołącz" (widoczność dla innych)
  const [isOpen, setIsOpen] = useState(false)
  const [openNote, setOpenNote] = useState('')

  // live state
  const walkersRef = useRef<Map<string, Walker>>(new Map())
  const [walkers, setWalkers] = useState<Walker[]>([])
  const [myTrack, setMyTrack] = useState<{ lat: number; lng: number }[]>([])
  const namesRef = useRef<Map<string, string>>(new Map())
  const [leaderboard, setLeaderboard] = useState<LeaderRow[]>([])
  const socketRef = useRef<LiveSocket | null>(null)
  const seqRef = useRef(0)
  const watchRef = useRef<number | null>(null)
  const lastSentRef = useRef(0)
  const [summary, setSummary] = useState<{ points: number; meters: number; steps: number; together: boolean; nature: boolean } | null>(null)
  const { steps, permissionNeeded, requestPermission, addMeters, reset: resetSteps } = useStepCounter()

  // uczestnicy z serwera (autorytatywna lista) + kick dla hosta
  const [walkDetail, setWalkDetail] = useState<WalkDetailInfo | null>(null)
  const [kickArmedId, setKickArmedId] = useState<string | null>(null)
  const [kickingId, setKickingId] = useState<string | null>(null)

  useEffect(() => {
    if (phase !== 'active' || !sessionId) return
    const load = () => { api.getWalkDetail(sessionId).then(setWalkDetail).catch(() => {}) }
    load()
    const id = window.setInterval(load, 15000)
    return () => window.clearInterval(id)
  }, [phase, sessionId])

  const kick = async (userId: string) => {
    if (kickArmedId !== userId) {
      setKickArmedId(userId)
      return
    }
    setKickingId(userId)
    try {
      await api.kickParticipant(sessionId, userId)
      const d = await api.getWalkDetail(sessionId)
      setWalkDetail(d)
    } catch {
      /* lista się nie zmieni — host spróbuje ponownie */
    } finally {
      setKickingId(null)
      setKickArmedId(null)
    }
  }

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

  const makeSock = (id: string) => {
    const sock = new LiveSocket({
      onOpen: () => { sock.subscribeSession(id); sock.subscribeLeaderboard() },
      onPingScored: onPing,
      onLeaderboard,
      onError: (m) => setError(m),
      onClose: () => { socketRef.current = null },
    })
    sock.connect()
    return sock
  }

  const connectAndStream = (id: string) => {
    socketRef.current?.close()
    walkersRef.current = new Map(); flush()
    seqRef.current = 0; setSec(0); setSummary(null); resetSteps(); setMyTrack([])
    socketRef.current = makeSock(id)
    startGps(id)
    setPhase('active')
  }

  // Kontrakt z ekranem Społeczności: ?session=<uuid> w URL = użytkownik już
  // dołączył do sesji przez API (POST join zrobiony wcześniej przez Community).
  // Tu tylko ustawiamy sessionId i startujemy spacer w trybie uczestnika —
  // bez tworzenia nowej sesji i bez join-by-code.
  useEffect(() => {
    const sid = searchParams.get('session')
    if (!sid || joinedViaQueryRef.current || phase !== 'idle') return
    joinedViaQueryRef.current = true
    setSessionId(sid); setJoinCode(null); setCodeInput('')
    connectAndStream(sid)
    const next = new URLSearchParams(searchParams)
    next.delete('session')
    setSearchParams(next, { replace: true })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase, searchParams])

  // Re-connect WS when app returns to foreground mid-walk
  useEffect(() => {
    if (phase !== 'active') return
    const onVisible = () => {
      if (document.visibilityState === 'visible' && !socketRef.current) {
        setError(null)
        socketRef.current = makeSock(sessionId)
      }
    }
    document.addEventListener('visibilitychange', onVisible)
    return () => document.removeEventListener('visibilitychange', onVisible)
  }, [phase, sessionId])

  const startWalk = async () => {
    if (busy) return
    setBusy(true); setError(null)
    try {
      const res = await apiRequest<WalkSession>('/walks', {
        method: 'POST',
        body: { is_open: isOpen, open_note: isOpen && openNote.trim() ? openNote.trim() : null },
      })
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
        const acc = pos.coords.accuracy
        // Drop poor-fix readings client-side: a wide accuracy radius drifts
        // several metres while standing still and would mint phantom points.
        if (typeof acc === 'number' && acc > 35) return
        const lat = pos.coords.latitude
        const lng = pos.coords.longitude
        // Ślad na mapę live — niezależny od throttle'u wysyłki pingów poniżej,
        // bo tylko rysuje trasę i nie wpływa na punktację (tę liczy serwer).
        setMyTrack((prev) => [...prev, { lat, lng }].slice(-500))
        const now = Date.now()
        // ~4 s cadence so genuine walking (>~1 m/s) clears the server's 5 m
        // jitter deadband, while stationary drift stays under it.
        if (now - lastSentRef.current < 4000) return
        lastSentRef.current = now
        seqRef.current += 1
        socketRef.current?.sendPing(id, seqRef.current, lat, lng, acc)
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
    const nat = mine?.nature ?? 1
    const tog = mine?.together ?? 1
    const finalPoints = Math.round(mine?.points ?? 0)
    setSummary({ points: finalPoints, meters: mine?.meters ?? 0, steps, together: tog > 1, nature: nat > 1 })
    if (sessionId) {
      const now = new Date()
      const hh = String(now.getHours()).padStart(2, '0')
      const mm = String(now.getMinutes()).padStart(2, '0')
      addWalk({
        id: sessionId || `w-${Date.now()}`,
        dateLabel: `Dziś • ${hh}:${mm}`,
        durationSec: sec,
        steps,
        points: finalPoints,
        withSomeone: walkers.length > 1,
        inNature: nat > 1,
        place: 'Spacer GPS',
        routeSeed: Math.abs(Math.round((mine?.meters ?? 0) * 1000)) || Date.now() % 100000,
        photos: [],
      })
    }
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
  const displayPoints = Math.round(mine?.points ?? 0)

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

              <Card className="mt-3 p-4">
                <label className="flex items-center justify-between gap-3">
                  <span className="text-sm font-bold text-ink">Pokaż innym, że idę — może ktoś dołączy 🌊</span>
                  <button
                    type="button"
                    role="switch"
                    aria-checked={isOpen}
                    aria-label="Pokaż innym, że idę"
                    onClick={() => setIsOpen((v) => !v)}
                    className={`relative h-7 w-12 shrink-0 rounded-full border border-white/70 transition-colors ${isOpen ? 'bg-sea' : 'bg-white/70'}`}
                  >
                    <span className={`absolute top-0.5 h-6 w-6 rounded-full bg-white shadow transition-transform ${isOpen ? 'translate-x-5' : 'translate-x-0.5'}`} />
                  </button>
                </label>
                {isOpen && (
                  <textarea
                    value={openNote}
                    onChange={(e) => setOpenNote(e.target.value.slice(0, 200))}
                    maxLength={200}
                    rows={2}
                    placeholder="np. chętnie pogadam po drodze"
                    className="mt-3 w-full resize-none rounded-xl border border-white/70 bg-white/80 px-3 py-2 text-sm outline-none"
                  />
                )}
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
                  <Stat label="punkty" value={displayPoints.toLocaleString('pl-PL')} accent />
                </div>
                <div className="mt-3 flex flex-wrap justify-center gap-2">
                  <Pill tone="leaf"><Leaf size={12} /> natura ×{mine?.nature ?? 1}</Pill>
                  <Pill tone="sea"><UsersThree size={12} /> we dwoje ×{mine?.together ?? 1}</Pill>
                  {combined > 1 && <Pill tone="sand">razem ×{combined.toFixed(1)}</Pill>}
                  <Pill tone="muted"><Footprints size={12} /> GPS</Pill>
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

              <Card className="mt-3 overflow-hidden p-2">
                <RealMap points={myTrack} live className="h-56" />
              </Card>

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

              {/* gospodarz widzi, kto dołączył, i może wyrzucić uczestnika */}
              {walkDetail?.hostId === currentUserId() &&
                walkDetail.participants.some((p) => p.userId !== currentUserId() && !p.leftAt) && (
                <Card className="mt-3 p-3">
                  <p className="text-xs font-bold uppercase tracking-wide text-muted">Uczestnicy Twojego spaceru</p>
                  <div className="mt-2 space-y-1.5">
                    {walkDetail.participants
                      .filter((p) => p.userId !== currentUserId() && !p.leftAt)
                      .map((p) => (
                        <div key={p.userId} className="flex items-center gap-2">
                          <span className="min-w-0 flex-1 truncate text-sm font-bold text-ink">{p.name}</span>
                          <button
                            type="button"
                            onClick={() => kick(p.userId)}
                            disabled={kickingId === p.userId}
                            className={`shrink-0 rounded-full px-3 py-1.5 text-xs font-bold transition active:scale-95 disabled:opacity-50 ${
                              kickArmedId === p.userId ? 'bg-rose-500/15 text-rose-600' : 'bg-white/70 text-muted'
                            }`}
                          >
                            {kickArmedId === p.userId ? 'Wyrzucić?' : 'Wyrzuć'}
                          </button>
                        </div>
                      ))}
                  </div>
                </Card>
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
                  <div className="font-display text-5xl font-bold text-sea">+{summary.points.toLocaleString('pl-PL')}</div>
                  <div className="text-sm font-bold text-muted">punktów zdobytych</div>
                  <div className="mt-2 flex justify-center gap-2">
                    {summary.nature && <Pill tone="leaf"><Leaf size={12} /> natura ×3</Pill>}
                    {summary.together && <Pill tone="sea"><UsersThree size={12} /> we dwoje ×1.5</Pill>}
                  </div>
                </div>
                <p className="mt-4 inline-flex items-center justify-center gap-1.5 text-sm font-bold text-[#2f7a45]"><HandHeart size={16} weight="fill" /> Jesteś coraz bliżej adopcji foki!</p>
              </Card>
              <RatingPanel sessionId={sessionId} />
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

/**
 * Panel ocen po spacerze (spec 2026-07-13): 👍 polecam / 👎 nie polecam per
 * współuczestnik; 👎 rozwija flagi problemowe (niepubliczne — moderacja).
 * Sesja musi być zakończona przez gospodarza; uczestnik, który wyszedł
 * wcześniej, widzi podpowiedź zamiast przycisków.
 */
function RatingPanel({ sessionId }: { sessionId: string }) {
  const myId = currentUserId()
  const [detail, setDetail] = useState<WalkDetailInfo | null>(null)
  const [verdicts, setVerdicts] = useState<Record<string, boolean>>({})
  const [flagFor, setFlagFor] = useState<string | null>(null)
  const [busyId, setBusyId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!sessionId) return
    api.getWalkDetail(sessionId).then(setDetail).catch(() => {})
    api
      .getMyWalkRatings(sessionId)
      .then((rs) => setVerdicts(Object.fromEntries(rs.map((r) => [r.userId, r.recommend]))))
      .catch(() => {})
  }, [sessionId])

  if (!sessionId || !detail) return null
  const others = detail.participants.filter((p) => p.userId !== myId)
  if (others.length === 0) return null

  if (detail.status !== 'finished') {
    return (
      <Card className="mt-4 p-4">
        <p className="text-sm text-muted">Ocenisz współuczestników, gdy gospodarz zakończy spacer.</p>
      </Card>
    )
  }

  const send = async (userId: string, recommend: boolean, flag?: RatingFlag) => {
    if (busyId) return
    setBusyId(userId)
    setError(null)
    try {
      await api.rateParticipant(sessionId, userId, recommend, flag)
      setVerdicts((v) => ({ ...v, [userId]: recommend }))
      setFlagFor(null)
    } catch {
      setError('Nie udało się zapisać oceny — spróbuj ponownie.')
    } finally {
      setBusyId(null)
    }
  }

  const FLAGS: { key: RatingFlag; label: string }[] = [
    { key: 'no_show', label: 'Nie pojawił(a) się' },
    { key: 'unsafe', label: 'Niepokojące zachowanie' },
    { key: 'spam', label: 'Spam / naciąganie' },
    { key: 'other', label: 'Inny problem' },
  ]

  return (
    <Card className="mt-4 p-4">
      <p className="text-xs font-bold uppercase tracking-wide text-muted">Jak było? Oceń współuczestników</p>
      <div className="mt-2 space-y-2">
        {others.map((p) => {
          const verdict = verdicts[p.userId]
          return (
            <div key={p.userId}>
              <div className="flex items-center gap-2">
                <span className="min-w-0 flex-1 truncate text-sm font-bold text-ink">{p.name}</span>
                <button
                  type="button"
                  onClick={() => send(p.userId, true)}
                  disabled={busyId === p.userId}
                  aria-label={`Polecam: ${p.name}`}
                  className={`grid h-9 w-9 shrink-0 place-items-center rounded-full transition active:scale-90 disabled:opacity-50 ${
                    verdict === true ? 'bg-leaf/20 text-leaf' : 'bg-white/70 text-muted'
                  }`}
                >
                  <ThumbsUp size={16} weight={verdict === true ? 'fill' : 'regular'} />
                </button>
                <button
                  type="button"
                  onClick={() => setFlagFor(flagFor === p.userId ? null : p.userId)}
                  disabled={busyId === p.userId}
                  aria-label={`Nie polecam: ${p.name}`}
                  className={`grid h-9 w-9 shrink-0 place-items-center rounded-full transition active:scale-90 disabled:opacity-50 ${
                    verdict === false ? 'bg-rose-500/15 text-rose-600' : 'bg-white/70 text-muted'
                  }`}
                >
                  <ThumbsDown size={16} weight={verdict === false ? 'fill' : 'regular'} />
                </button>
              </div>
              {flagFor === p.userId && (
                <div className="mt-1.5 flex flex-wrap gap-1.5">
                  <button
                    type="button"
                    onClick={() => send(p.userId, false)}
                    className="rounded-full bg-white/70 px-3 py-1 text-xs font-bold text-muted transition active:scale-95"
                  >
                    Po prostu nie polecam
                  </button>
                  {FLAGS.map((f) => (
                    <button
                      key={f.key}
                      type="button"
                      onClick={() => send(p.userId, false, f.key)}
                      className="rounded-full bg-rose-500/10 px-3 py-1 text-xs font-bold text-rose-600 transition active:scale-95"
                    >
                      {f.label}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )
        })}
      </div>
      {error && <p className="mt-2 text-xs font-semibold text-rose-600">{error}</p>}
      <p className="mt-2 text-[11px] text-muted">
        Oceny budują zaufanie w SeaSteps. Zgłoszenia problemów nie są publiczne — trafiają do moderacji.
      </p>
    </Card>
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
