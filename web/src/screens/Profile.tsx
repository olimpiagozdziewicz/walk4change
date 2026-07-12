import { useEffect, useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { motion } from 'motion/react'
import { Footprints, CalendarHeart, Recycle, GearSix, PencilSimple, Check, SignOut, Prohibit } from '@phosphor-icons/react'
import { Card, Pill, PrimaryButton } from '../components/ui'
import { Glyph } from '../components/Glyph'
import { FootstepTrail } from '../components/Footsteps'
import { Avatar } from '../components/Avatar'
import { api, INTEREST_OPTIONS, type Profile as ProfileT, type EcoReport, type RedemptionItem, type Reward, type BlockedUser } from '../lib/api'
import { getInterests, saveInterests } from '../lib/interests'
import { getGender, saveGender, type Gender } from '../lib/settings'
import { logout } from '../lib/auth'

export function Profile() {
  const nav = useNavigate()
  const [p, setP] = useState<ProfileT | null>(null)
  const [interests, setInterests] = useState<string[]>(getInterests())
  const [editing, setEditing] = useState(false)
  const [interestsSaving, setInterestsSaving] = useState(false)
  const [gender, setGender] = useState<Gender>(getGender())
  const [editingName, setEditingName] = useState(false)
  const [nameInput, setNameInput] = useState('')
  const [nameSaving, setNameSaving] = useState(false)
  const [ecoReports, setEcoReports] = useState<EcoReport[]>([])
  const [blocked, setBlocked] = useState<BlockedUser[]>([])
  const [unblockingId, setUnblockingId] = useState<string | null>(null)
  const [redemptions, setRedemptions] = useState<RedemptionItem[]>([])
  const [rewardTitles, setRewardTitles] = useState<Record<string, string>>({})
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)

  const pickGender = (g: Gender) => {
    setGender(g)
    saveGender(g)
  }

  const loadProfile = () => {
    setLoading(true)
    setLoadError(false)
    ;(async () => {
      try {
        const profile = await api.getProfile()
        // backend to źródło prawdy o zainteresowaniach — localStorage to tylko cache offline
        setInterests(profile.interests)
        saveInterests(profile.interests)

        const [counters, reports] = await Promise.all([
          api.getProfileCounters(),
          api.getMyEcoReports(),
        ])
        setEcoReports(reports)
        setP({
          ...profile,
          stats: { walks: counters.totalWalks, events: 0, ecoReports: reports.length },
          badges: buildBadges({ totalWalks: counters.totalWalks, streakDays: counters.streakDays, ecoReports: reports.length }),
        })
      } catch {
        setLoadError(true)
      } finally {
        setLoading(false)
      }
    })()
  }

  const unblock = async (id: string) => {
    if (unblockingId) return
    setUnblockingId(id)
    try {
      await api.unblockUser(id)
      setBlocked((cur) => cur.filter((b) => b.id !== id))
    } catch {
      /* lista się nie zmieni — user spróbuje ponownie */
    } finally {
      setUnblockingId(null)
    }
  }

  useEffect(() => {
    loadProfile()
    // Zablokowani — best-effort, sekcja znika przy błędzie/braku.
    api.getBlockedUsers().then(setBlocked).catch(() => {})
    // Moje kody nagród — best-effort, sekcja znika przy błędzie/braku.
    api.getMyRedemptions().then(setRedemptions).catch(() => {})
    api
      .getRewards()
      .then((rs: Reward[]) => setRewardTitles(Object.fromEntries(rs.map((r) => [r.id, r.title]))))
      .catch(() => {})
  }, [])

  const toggle = (tag: string) =>
    setInterests((cur) => (cur.includes(tag) ? cur.filter((t) => t !== tag) : [...cur, tag]))

  const finishEdit = async () => {
    setEditing(false)
    saveInterests(interests) // cache offline natychmiast
    setInterestsSaving(true)
    try {
      const updated = await api.patchProfile({ interests })
      setP((cur) => (cur ? { ...cur, interests: updated.interests } : cur))
    } catch {
      /* backend nieosiągalny — zostaje lokalny cache, spróbujemy przy kolejnej edycji */
    } finally {
      setInterestsSaving(false)
    }
  }

  const saveName = async () => {
    const name = nameInput.trim()
    if (!name || !p) return
    setNameSaving(true)
    try {
      const updated = await api.patchProfile({ display_name: name })
      setP({ ...p, name: updated.name })
    } catch { /* ignore */ } finally {
      setNameSaving(false)
      setEditingName(false)
    }
  }

  if (loading) {
    return (
      <div className="px-5 pt-5">
        <Card className="p-6 text-center text-sm font-semibold text-muted">Wczytywanie profilu…</Card>
      </div>
    )
  }

  if (loadError || !p) {
    return (
      <div className="px-5 pt-5">
        <Card className="flex flex-col items-center gap-3 p-6 text-center">
          <p className="text-sm font-semibold text-rose-600">Nie udało się wczytać profilu.</p>
          <PrimaryButton onClick={loadProfile}>Spróbuj ponownie</PrimaryButton>
        </Card>
      </div>
    )
  }

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
          {editingName ? (
            <div className="mt-3 flex items-center justify-center gap-2">
              <input
                autoFocus
                value={nameInput}
                onChange={(e) => setNameInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && saveName()}
                className="rounded-xl border border-sea/30 bg-white/80 px-3 py-1.5 text-center font-display text-xl font-bold text-ink outline-none focus:ring-2 focus:ring-sea/30"
              />
              <button onClick={saveName} disabled={nameSaving} className="grid h-8 w-8 place-items-center rounded-full bg-sea/15 text-sea disabled:opacity-50">
                <Check size={15} weight="bold" />
              </button>
            </div>
          ) : (
            <button
              onClick={() => { setNameInput(p.name); setEditingName(true) }}
              className="mt-3 inline-flex items-center gap-1.5 font-display text-2xl font-bold text-ink"
            >
              {p.name} <PencilSimple size={16} className="text-muted" />
            </button>
          )}

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
              disabled={interestsSaving}
              className="inline-flex items-center gap-1 rounded-full bg-sea/10 px-2.5 py-1 text-xs font-bold text-deep transition active:scale-95 disabled:opacity-60"
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
        {p.badges.length > 0 ? (
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
        ) : (
          <p className="text-sm text-muted">Odznaki pojawią się wraz z aktywnością.</p>
        )}

        {/* moje nagrody (kody z wymiany punktów) */}
        {redemptions.length > 0 && (
          <>
            <h2 className="mb-3 mt-6 font-display text-lg font-bold text-ink">Moje nagrody</h2>
            <div className="space-y-2.5">
              {redemptions.map((r) => (
                <Card key={r.id} className="flex items-center gap-3 p-3.5">
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-bold text-ink">{rewardTitles[r.rewardId] ?? 'Nagroda'}</div>
                    <div className="font-mono text-base font-bold tracking-widest text-deep">{r.code}</div>
                  </div>
                  <Pill tone={r.status === 'redeemed' ? 'muted' : r.status === 'expired' ? 'sand' : 'leaf'}>
                    {r.status === 'redeemed' ? 'wykorzystany' : r.status === 'expired' ? 'wygasł' : 'do użycia'}
                  </Pill>
                </Card>
              ))}
            </div>
          </>
        )}

        {/* moje zgłoszenia eko */}
        {ecoReports.length > 0 && (
          <>
            <h2 className="mb-3 mt-6 font-display text-lg font-bold text-ink">Moje zgłoszenia eko</h2>
            <div className="space-y-2.5">
              {ecoReports.map((r) => {
                const thumb = r.photoAfterUrl || r.photoUrl || r.photoBeforeUrl
                return (
                  <Card key={r.id} className="flex items-center gap-3 p-3.5">
                    {thumb ? (
                      <img src={thumb} alt="" className="h-12 w-12 shrink-0 rounded-xl object-cover" />
                    ) : (
                      <div className="grid h-10 w-10 place-items-center rounded-xl bg-leaf/15 text-leaf">
                        <Recycle size={18} />
                      </div>
                    )}
                    <div className="min-w-0 flex-1">
                      <div className="text-sm font-bold text-ink">
                        {r.kind === 'cleanup' ? 'Posprzątane' : 'Zgłoszenie'}{r.type ? ` • ${r.type}` : ''}
                      </div>
                      <div className="truncate text-xs text-muted">{r.description || '—'}</div>
                    </div>
                  </Card>
                )
              })}
            </div>
          </>
        )}

        {/* zablokowani */}
        {blocked.length > 0 && (
          <>
            <h2 className="mb-3 mt-6 flex items-center gap-2 font-display text-lg font-bold text-ink">
              <Prohibit size={18} className="text-muted" /> Zablokowani
            </h2>
            <div className="space-y-2.5">
              {blocked.map((b) => (
                <Card key={b.id} className="flex items-center gap-3 p-3.5">
                  <Avatar name={b.name} size={36} />
                  <span className="min-w-0 flex-1 text-sm font-bold text-ink">{b.name}</span>
                  <button
                    type="button"
                    onClick={() => unblock(b.id)}
                    disabled={unblockingId === b.id}
                    className="shrink-0 rounded-full bg-sea/10 px-3 py-1.5 text-xs font-bold text-deep transition active:scale-95 disabled:opacity-50"
                  >
                    Odblokuj
                  </button>
                </Card>
              ))}
            </div>
          </>
        )}

        <button
          onClick={async () => {
            await logout()
            nav('/login')
          }}
          className="mt-8 flex w-full items-center justify-center gap-2 rounded-2xl border border-white/70 bg-white/60 py-3 text-sm font-bold text-muted transition active:scale-[0.98]"
        >
          <SignOut size={16} /> Wyloguj się
        </button>

        <p className="mt-4 text-center text-xs text-muted">
          <a href="/privacy.html" target="_blank" rel="noopener" className="underline transition hover:text-sea">
            Polityka Prywatności
          </a>
        </p>

      </div>
    </div>
  )
}

/**
 * Odznaki nie mają backendu — wyprowadzamy je z realnych liczników, tylko
 * gdy liczba faktycznie na nie zasługuje. Bez backendu na odznaki = brak
 * wymyślonych danych, po prostu pusta lista.
 */
function buildBadges(counters: { totalWalks: number; streakDays: number; ecoReports: number }): ProfileT['badges'] {
  const badges: ProfileT['badges'] = []
  if (counters.totalWalks > 0) badges.push({ id: 'firststep', label: 'Pierwszy spacer', iconKey: 'firststep' })
  if (counters.streakDays > 0) badges.push({ id: 'streak', label: `Seria ${counters.streakDays} dni`, iconKey: 'streak' })
  if (counters.ecoReports > 0) badges.push({ id: 'shore', label: 'Strażnik Bałtyku', iconKey: 'shore' })
  return badges
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
