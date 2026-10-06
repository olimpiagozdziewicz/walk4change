import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { motion } from 'motion/react'
import { Clock, Footprints, MapPin, Leaf, UsersThree, Plus, MapTrifold, Star, PawPrint, CaretDown } from '@phosphor-icons/react'
import { ScreenHeader, Card, Pill, PrimaryButton } from '../components/ui'
import { RouteMap } from '../components/RouteMap'
import { RealMap } from '../components/RealMap'
import { api, type ServerWalk } from '../lib/api'
import { currentUserId } from '../lib/auth'
import { getWalks, type SavedWalk } from '../lib/walks'

function fmtMin(sec: number) {
  const m = Math.floor(sec / 60)
  return `${m} min`
}

/** started_at → "10.07.2026 • 14:32" (pl-PL). */
function fmtDate(iso: string) {
  const d = new Date(iso)
  const date = d.toLocaleDateString('pl-PL', { day: '2-digit', month: '2-digit', year: 'numeric' })
  const time = d.toLocaleTimeString('pl-PL', { hour: '2-digit', minute: '2-digit' })
  return `${date} • ${time}`
}

function fmtKm(meters: number) {
  return `${(meters / 1000).toFixed(1)} km`
}

function fmtDurationMin(startedAt: string, endedAt: string | null): number | null {
  if (!endedAt) return null
  const ms = new Date(endedAt).getTime() - new Date(startedAt).getTime()
  if (!Number.isFinite(ms) || ms < 0) return null
  return Math.round(ms / 60000)
}

/** Polska odmiana: "z 1 osobą", "z 2 osobami"… — instrumental liczby mnogiej się nie zmienia od 2 wzwyż. */
function companionsLabel(n: number): string | null {
  if (n <= 0) return null
  return n === 1 ? 'z 1 osobą' : `z ${n} osobami`
}

function Photo({ src }: { src: string }) {
  const isImg = src.startsWith('data:') || src.startsWith('http')
  if (isImg) return <img src={src} alt="" className="h-16 w-16 shrink-0 rounded-2xl object-cover" />
  return (
    <div className="grid h-16 w-16 shrink-0 place-items-center rounded-2xl bg-gradient-to-br from-sea/15 to-leaf/15 text-2xl">
      {src}
    </div>
  )
}

export function History() {
  const nav = useNavigate()
  const [source, setSource] = useState<'loading' | 'server' | 'local'>('loading')
  const [serverWalks, setServerWalks] = useState<ServerWalk[]>([])
  const [localWalks, setLocalWalks] = useState<SavedWalk[]>([])

  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [tracks, setTracks] = useState<Record<string, { lat: number; lng: number }[]>>({})
  const [trackLoadingId, setTrackLoadingId] = useState<string | null>(null)

  useEffect(() => {
    api
      .getMyWalks()
      .then((ws) => { setServerWalks(ws); setSource('server') })
      .catch(() => { setLocalWalks(getWalks()); setSource('local') })
  }, [])

  const toggleExpand = (sessionId: string) => {
    if (expandedId === sessionId) { setExpandedId(null); return }
    setExpandedId(sessionId)
    if (tracks[sessionId] !== undefined) return // już w cache — nie pobieraj drugi raz
    setTrackLoadingId(sessionId)
    const myId = currentUserId()
    api
      .getWalkTrack(sessionId)
      .then((points) => {
        const mine = points
          .filter((p) => !myId || p.userId === myId)
          .sort((a, b) => a.seq - b.seq)
          .map((p) => ({ lat: p.lat, lng: p.lng }))

        // Bezpiecznik: przy brakującym/starym `ss-uid` nie wolno zamieniać
        // niepustego tracka z backendu w pustą mapę. Endpoint jest member-only,
        // więc użytkownik i tak ma prawo widzieć punkty tej sesji.
        const fallback = points
          .sort((a, b) => a.seq - b.seq)
          .map((p) => ({ lat: p.lat, lng: p.lng }))
        setTracks((prev) => ({ ...prev, [sessionId]: mine.length > 0 ? mine : fallback }))
      })
      .catch(() => setTracks((prev) => ({ ...prev, [sessionId]: [] })))
      .finally(() => setTrackLoadingId(null))
  }

  // auto-ulubione (tylko dla starych, lokalnych wpisów — mają nazwę miejsca)
  const counts = localWalks.reduce<Record<string, number>>((m, w) => {
    m[w.place] = (m[w.place] || 0) + 1
    return m
  }, {})
  const favorites = Object.keys(counts).filter((p) => counts[p] >= 2)
  const favSet = new Set(favorites)

  return (
    <div>
      <ScreenHeader title="Moje spacery" icon={<MapTrifold size={22} />} subtitle="Twoje trasy, punkty i zdjęcia z drogi." />

      <div className="space-y-4 px-5 pt-2">
        <PrimaryButton onClick={() => nav('/walk')} className="w-full">
          <Plus size={18} /> Nowy spacer
        </PrimaryButton>

        {source === 'loading' && (
          <p className="px-1 text-sm font-semibold text-muted">Wczytywanie spacerów…</p>
        )}

        {/* ── Główne źródło: spacery z serwera ── */}
        {source === 'server' && serverWalks.map((w, i) => {
          const durationMin = fmtDurationMin(w.startedAt, w.endedAt)
          const companions = companionsLabel(w.companions)
          const expanded = expandedId === w.sessionId
          const track = tracks[w.sessionId]
          const loadingTrack = trackLoadingId === w.sessionId

          return (
            <motion.div key={w.sessionId} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.07 }}>
              <Card className="overflow-hidden" onClick={() => toggleExpand(w.sessionId)}>
                <div className="p-4">
                  <div className="flex items-start justify-between gap-2">
                    <div>
                      <div className="flex items-center gap-1.5 font-display text-lg font-bold leading-tight text-ink">
                        Spacer GPS
                        {w.isHost && <Pill tone="sand">host</Pill>}
                      </div>
                      <div className="text-xs font-bold text-muted">{fmtDate(w.startedAt)}</div>
                    </div>
                    <span className="font-display text-xl font-bold text-sea">+{w.points}</span>
                  </div>

                  <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs font-bold text-muted">
                    <span className="inline-flex items-center gap-1">
                      <MapPin size={13} /> {fmtKm(w.meters)}
                    </span>
                    {durationMin != null && (
                      <span className="inline-flex items-center gap-1">
                        <Clock size={13} /> {durationMin} min
                      </span>
                    )}
                    {companions && (
                      <span className="inline-flex items-center gap-1">
                        <UsersThree size={13} /> {companions}
                      </span>
                    )}
                    {w.withDog && (
                      <Pill tone="sand">
                        <PawPrint size={12} /> z psem
                      </Pill>
                    )}
                    <CaretDown size={13} className={`ml-auto transition-transform ${expanded ? 'rotate-180' : ''}`} />
                  </div>
                </div>

                {expanded && (
                  <div className="border-t border-white/60 p-3" onClick={(e) => e.stopPropagation()}>
                    {loadingTrack ? (
                      <div className="grid h-56 place-items-center rounded-3xl bg-white/60 text-sm font-bold text-muted">
                        Wczytywanie trasy…
                      </div>
                    ) : (track?.length ?? 0) > 0 ? (
                      <RealMap points={track ?? []} className="h-56" />
                    ) : (
                      <div className="grid h-56 place-items-center rounded-3xl bg-white/60 text-sm font-bold text-muted">
                        Brak zapisanej trasy GPS
                      </div>
                    )}
                  </div>
                )}
              </Card>
            </motion.div>
          )
        })}

        {/* ── Fallback: stare wpisy lokalne (localStorage), gdy serwer nie odpowiedział ── */}
        {source === 'local' && (
          <>
            {favorites.length > 0 && (
              <div>
                <h2 className="mb-2 flex items-center gap-2 font-display text-base font-bold text-ink">
                  <Star size={16} weight="fill" className="text-sand" /> Ulubione miejsca
                </h2>
                <div className="flex flex-wrap gap-2">
                  {favorites.map((p) => (
                    <span key={p} className="inline-flex items-center gap-1.5 rounded-full bg-sand/20 px-3 py-1.5 text-xs font-bold text-[#8a6418]">
                      <Star size={12} weight="fill" /> {p}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {localWalks.map((w, i) => (
              <motion.div key={w.id} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.07 }}>
                <Card className="overflow-hidden">
                  <RouteMap seed={w.routeSeed} height={150} />
                  <div className="p-4">
                    <div className="flex items-start justify-between gap-2">
                      <div>
                        <div className="flex items-center gap-1.5 font-display text-lg font-bold leading-tight text-ink">
                          {w.place}
                          {favSet.has(w.place) && <Star size={15} weight="fill" className="text-sand" />}
                        </div>
                        <div className="mt-0.5 flex items-center gap-1.5 text-xs font-bold text-muted">
                          <span>{w.dateLabel}</span>
                          <Pill tone="muted">zapis lokalny</Pill>
                        </div>
                      </div>
                      <span className="font-display text-xl font-bold text-sea">+{w.points}</span>
                    </div>

                    <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-xs font-bold text-muted">
                      <span className="inline-flex items-center gap-1">
                        <Clock size={13} /> {fmtMin(w.durationSec)}
                      </span>
                      <span className="inline-flex items-center gap-1">
                        <Footprints size={13} /> {w.steps.toLocaleString('pl-PL')} kroków
                      </span>
                      <span className="inline-flex items-center gap-1">
                        <MapPin size={13} /> trasa zapisana
                      </span>
                    </div>

                    {(w.inNature || w.withSomeone || w.withDog) && (
                      <div className="mt-2.5 flex flex-wrap gap-1.5">
                        {w.inNature && (
                          <Pill tone="leaf">
                            <Leaf size={12} /> natura ×3
                          </Pill>
                        )}
                        {w.withSomeone && (
                          <Pill tone="sea">
                            <UsersThree size={12} /> z kimś ×1.5
                          </Pill>
                        )}
                        {w.withDog && (
                          <Pill tone="sand">
                            <PawPrint size={12} /> z psem
                          </Pill>
                        )}
                      </div>
                    )}

                    {w.photos.length > 0 && (
                      <div className="no-scrollbar mt-3 flex gap-2 overflow-x-auto">
                        {w.photos.map((p, idx) => (
                          <Photo key={idx} src={p} />
                        ))}
                      </div>
                    )}
                  </div>
                </Card>
              </motion.div>
            ))}
          </>
        )}
      </div>
    </div>
  )
}
