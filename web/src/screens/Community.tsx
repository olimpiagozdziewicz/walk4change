import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { motion } from 'motion/react'
import {
  MapPin,
  Clock,
  Trophy,
  UsersThree,
  Footprints,
  ChatCircle,
  Buildings,
  Heart,
  UserPlus,
  MagnifyingGlass,
  PaperPlaneRight,
  Prohibit,
  ThumbsUp,
  X,
} from '@phosphor-icons/react'
import { ScreenHeader, Card, Pill, PrimaryButton, SoftButton, SoonBadge, DemoBanner } from '../components/ui'
import { Avatar } from '../components/Avatar'
import { useMode } from '../lib/mode'
import { getInterests } from '../lib/interests'
import { ApiError } from '../lib/http'
import {
  api,
  type CommunityWalk,
  type LeaderboardRow,
  type TeamRow,
  type MatchPerson,
  type OpenWalkItem,
  type Conversation,
  type FriendsData,
  type UserSearchResult,
  type EcoReport,
  type EcoComment,
} from '../lib/api'

function minutesAgo(iso: string): number {
  const diffMs = Date.now() - new Date(iso).getTime()
  return Math.max(0, Math.round(diffMs / 60000))
}

function timeAgo(iso: string): string {
  const mins = minutesAgo(iso)
  if (mins < 1) return 'przed chwilą'
  if (mins < 60) return `${mins} min temu`
  if (mins < 1440) return `${Math.round(mins / 60)} godz. temu`
  return new Date(iso).toLocaleDateString('pl-PL', { day: 'numeric', month: 'short' })
}

const EMPTY_FRIENDS: FriendsData = { accepted: [], incoming: [], outgoing: [] }

/** Zakładki trybu solo — 8 sekcji w jednym scrollu robiło bałagan. */
type CommunityTab = 'feed' | 'friends' | 'ranking'

export function Community() {
  const navigate = useNavigate()
  const { mode } = useMode()
  const isTeam = mode === 'team'
  const [tab, setTab] = useState<CommunityTab>('feed')
  const [walks, setWalks] = useState<CommunityWalk[]>([])
  const [board, setBoard] = useState<LeaderboardRow[]>([])
  const [teamBoard, setTeamBoard] = useState<TeamRow[]>([])
  const [matches, setMatches] = useState<MatchPerson[]>([])
  const myInterests = getInterests()

  // ── Na spacerze teraz ──
  const [openWalks, setOpenWalks] = useState<OpenWalkItem[]>([])
  const [joiningId, setJoiningId] = useState<string | null>(null)
  const [joinErrors, setJoinErrors] = useState<Record<string, string>>({})

  // ── Wiadomości ──
  const [conversations, setConversations] = useState<Conversation[]>([])

  // ── Feed eko ──
  const [feed, setFeed] = useState<EcoReport[]>([])
  const [openComments, setOpenComments] = useState<Set<string>>(new Set())
  const [comments, setComments] = useState<Record<string, EcoComment[]>>({})
  const [commentDraft, setCommentDraft] = useState<Record<string, string>>({})
  const [commentSending, setCommentSending] = useState<string | null>(null)

  const toggleLike = async (reportId: string) => {
    // optymistycznie — cofamy przy błędzie
    setFeed((cur) =>
      cur.map((r) =>
        r.id === reportId
          ? { ...r, likedByMe: !r.likedByMe, likeCount: (r.likeCount ?? 0) + (r.likedByMe ? -1 : 1) }
          : r,
      ),
    )
    try {
      const { liked, likeCount } = await api.toggleEcoLike(reportId)
      setFeed((cur) => cur.map((r) => (r.id === reportId ? { ...r, likedByMe: liked, likeCount } : r)))
    } catch {
      setFeed((cur) =>
        cur.map((r) =>
          r.id === reportId
            ? { ...r, likedByMe: !r.likedByMe, likeCount: (r.likeCount ?? 0) + (r.likedByMe ? -1 : 1) }
            : r,
        ),
      )
    }
  }

  const toggleCommentsPanel = (reportId: string) => {
    setOpenComments((cur) => {
      const next = new Set(cur)
      if (next.has(reportId)) {
        next.delete(reportId)
      } else {
        next.add(reportId)
        if (!comments[reportId]) {
          api.getEcoComments(reportId)
            .then((list) => setComments((c) => ({ ...c, [reportId]: list })))
            .catch(() => {})
        }
      }
      return next
    })
  }

  const sendComment = async (reportId: string) => {
    const body = (commentDraft[reportId] ?? '').trim()
    if (!body || commentSending) return
    setCommentSending(reportId)
    try {
      const added = await api.addEcoComment(reportId, body)
      if (added) {
        setComments((c) => ({ ...c, [reportId]: [...(c[reportId] ?? []), added] }))
        setCommentDraft((d) => ({ ...d, [reportId]: '' }))
        setFeed((cur) =>
          cur.map((r) => (r.id === reportId ? { ...r, commentCount: (r.commentCount ?? 0) + 1 } : r)),
        )
      }
    } catch {
      /* zostaje draft — user spróbuje znowu */
    } finally {
      setCommentSending(null)
    }
  }

  // ── Znajomi ──
  const [friends, setFriends] = useState<FriendsData>(EMPTY_FRIENDS)
  const [respondingId, setRespondingId] = useState<string | null>(null)
  // usuwanie znajomego: pierwszy klik uzbraja ("Na pewno?"), drugi usuwa
  const [unfriendArmedId, setUnfriendArmedId] = useState<string | null>(null)
  const [unfriendingId, setUnfriendingId] = useState<string | null>(null)

  const unfriend = async (id: string) => {
    if (unfriendArmedId !== id) {
      setUnfriendArmedId(id)
      return
    }
    setUnfriendingId(id)
    try {
      await api.removeFriend(id)
      loadFriends()
    } catch {
      /* lista się nie zmieni — user spróbuje ponownie */
    } finally {
      setUnfriendingId(null)
      setUnfriendArmedId(null)
    }
  }

  // blokada: pierwszy klik uzbraja ("Zablokować?"), drugi blokuje na stałe
  const [blockArmedId, setBlockArmedId] = useState<string | null>(null)
  const [blockingId, setBlockingId] = useState<string | null>(null)

  const block = async (id: string) => {
    if (blockArmedId !== id) {
      setBlockArmedId(id)
      setUnfriendArmedId(null)
      return
    }
    setBlockingId(id)
    try {
      await api.blockUser(id)
      loadFriends()
    } catch {
      /* lista się nie zmieni — user spróbuje ponownie */
    } finally {
      setBlockingId(null)
      setBlockArmedId(null)
    }
  }

  // ── Znajdź ludzi ──
  const [query, setQuery] = useState('')
  const [searchResults, setSearchResults] = useState<UserSearchResult[]>([])
  const [searching, setSearching] = useState(false)
  const [sendingId, setSendingId] = useState<string | null>(null)
  const [sentIds, setSentIds] = useState<Set<string>>(new Set())
  const [conflictIds, setConflictIds] = useState<Set<string>>(new Set())

  useEffect(() => {
    api.getCommunityWalks().then(setWalks).catch(() => {})
    api.getLeaderboard().then(setBoard).catch(() => {})
    api.getTeamLeaderboard().then(setTeamBoard).catch(() => {})
    api.getMatches().then(setMatches).catch(() => {})
  }, [])

  const loadOpenWalks = () => {
    api.getOpenWalks().then(setOpenWalks).catch(() => {})
  }
  const loadFriends = () => {
    api.getFriends().then(setFriends).catch(() => {})
  }

  useEffect(() => {
    if (isTeam) return
    loadOpenWalks()
    api.getConversations().then(setConversations).catch(() => {})
    loadFriends()
    api.getEcoReports().then(setFeed).catch(() => {})
    const id = window.setInterval(loadOpenWalks, 30000)
    return () => window.clearInterval(id)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isTeam])

  // debounced search
  useEffect(() => {
    const q = query.trim()
    if (q.length < 2) {
      setSearchResults([])
      setSearching(false)
      return
    }
    setSearching(true)
    const t = window.setTimeout(() => {
      api
        .searchUsers(q)
        .then(setSearchResults)
        .catch(() => setSearchResults([]))
        .finally(() => setSearching(false))
    }, 400)
    return () => window.clearTimeout(t)
  }, [query])

  const knownIds = useMemo(() => {
    const s = new Set<string>()
    friends.accepted.forEach((f) => s.add(f.id))
    friends.incoming.forEach((r) => s.add(r.user.id))
    friends.outgoing.forEach((r) => s.add(r.user.id))
    return s
  }, [friends])

  const visibleResults = searchResults.filter((r) => !knownIds.has(r.id))

  // kropka na zakładce Znajomi: nieprzeczytane wiadomości + zaproszenia do przyjęcia
  const friendsBadge =
    conversations.reduce((sum, c) => sum + c.unread, 0) + friends.incoming.length

  const joinWalk = async (sessionId: string) => {
    if (joiningId) return
    setJoiningId(sessionId)
    setJoinErrors((e) => ({ ...e, [sessionId]: '' }))
    try {
      await api.joinOpenWalk(sessionId)
      navigate(`/walk?session=${sessionId}`)
    } catch (err) {
      const msg =
        err instanceof ApiError && err.status === 409
          ? 'Komplet uczestników'
          : err instanceof ApiError && err.code === 'EMAIL_NOT_VERIFIED'
            ? 'Potwierdź e-mail w Profilu, aby dołączać do otwartych spacerów.'
            : 'Nie udało się dołączyć.'
      setJoinErrors((e) => ({ ...e, [sessionId]: msg }))
    } finally {
      setJoiningId(null)
    }
  }

  const respond = async (requestId: string, accept: boolean) => {
    if (respondingId) return
    setRespondingId(requestId)
    try {
      await api.respondFriendRequest(requestId, accept)
      loadFriends()
    } catch {
      /* best-effort — lista po prostu się nie zmieni */
    } finally {
      setRespondingId(null)
    }
  }

  const addFriend = async (id: string) => {
    if (sendingId) return
    setSendingId(id)
    try {
      await api.sendFriendRequest(id)
      setSentIds((s) => new Set(s).add(id))
    } catch (err) {
      if (err instanceof ApiError && err.status === 409) {
        setConflictIds((s) => new Set(s).add(id))
      }
    } finally {
      setSendingId(null)
    }
  }

  return (
    <div>
      <ScreenHeader
        title={isTeam ? 'Zespół' : 'Społeczność'}
        icon={isTeam ? <Buildings size={22} /> : <UsersThree size={22} />}
        subtitle={isTeam ? 'Wspólne spacery działów i ranking zespołów.' : 'Dopasuj się do ludzi, umów się na spacer albo rozmowę.'}
      />

      <div className="space-y-4 px-5 pt-2">
        <DemoBanner>
          {isTeam
            ? 'To demo wersji firmowej — program firmowy projektujemy indywidualnie dla każdej firmy.'
            : 'Znajomi, czat i dołączanie do spacerów już działają. Dopasowania i umawianie spacerów — wkrótce.'}
        </DemoBanner>

        {!isTeam && (
          <>
            {/* zakładki */}
            <div className="inline-flex w-full rounded-2xl bg-white/70 p-1 text-sm font-bold ring-1 ring-white/60">
              <button
                onClick={() => setTab('feed')}
                className={`flex flex-1 items-center justify-center gap-1.5 rounded-xl py-2.5 transition ${
                  tab === 'feed' ? 'bg-gradient-to-br from-sea to-deep text-white shadow' : 'text-muted'
                }`}
              >
                <Heart size={15} /> Tablica
              </button>
              <button
                onClick={() => setTab('friends')}
                className={`relative flex flex-1 items-center justify-center gap-1.5 rounded-xl py-2.5 transition ${
                  tab === 'friends' ? 'bg-gradient-to-br from-sea to-deep text-white shadow' : 'text-muted'
                }`}
              >
                <UsersThree size={15} /> Znajomi
                {friendsBadge > 0 && (
                  <span className="absolute -right-0.5 -top-0.5 grid h-4 min-w-4 place-items-center rounded-full bg-rose-500 px-1 text-[10px] font-bold text-white">
                    {friendsBadge}
                  </span>
                )}
              </button>
              <button
                onClick={() => setTab('ranking')}
                className={`flex flex-1 items-center justify-center gap-1.5 rounded-xl py-2.5 transition ${
                  tab === 'ranking' ? 'bg-gradient-to-br from-sea to-deep text-white shadow' : 'text-muted'
                }`}
              >
                <Trophy size={15} /> Ranking
              </button>
            </div>

            {tab === 'feed' && (
            <>
            {/* ── Na spacerze teraz ── */}
            <section>
              <h2 className="mb-3 flex items-center gap-2 font-display text-lg font-bold text-ink">
                <Footprints size={18} className="text-sea" /> Na spacerze teraz
              </h2>
              {openWalks.length === 0 ? (
                <Card className="p-4">
                  <p className="text-sm text-muted">Nikt teraz nie spaceruje z otwartym zaproszeniem. Wyjdź na spacer i pokaż się innym!</p>
                </Card>
              ) : (
                <div className="space-y-3">
                  {openWalks.map((w, i) => {
                    const mins = minutesAgo(w.startedAt)
                    const err = joinErrors[w.sessionId]
                    return (
                      <motion.div
                        key={w.sessionId}
                        initial={{ opacity: 0, y: 14 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ delay: i * 0.06 }}
                      >
                        <Card className="p-4">
                          <div className="flex items-center gap-3">
                            <Avatar name={w.hostName} size={48} />
                            <div className="min-w-0 flex-1">
                              <div className="flex flex-wrap items-center gap-2">
                                <span className="font-display text-lg font-bold text-ink">{w.hostName}</span>
                                {w.hostRatingTotal >= 3 && (
                                  <Pill tone="leaf">
                                    <ThumbsUp size={12} weight="fill" /> {w.hostRecommendCount}/{w.hostRatingTotal}
                                  </Pill>
                                )}
                              </div>
                              <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs font-bold text-muted">
                                <span className="inline-flex items-center gap-1">
                                  <UsersThree size={12} /> {w.participants} uczestników
                                </span>
                                <span className="inline-flex items-center gap-1">
                                  <Clock size={12} /> {mins < 1 ? 'przed chwilą' : `${mins} min temu`}
                                </span>
                              </div>
                            </div>
                          </div>
                          {w.note && <p className="mt-2.5 text-sm text-muted">„{w.note}"</p>}
                          {err && <p className="mt-2 text-sm font-semibold text-rose-600">{err}</p>}
                          <PrimaryButton
                            onClick={() => joinWalk(w.sessionId)}
                            disabled={joiningId === w.sessionId}
                            className="mt-3 w-full py-2.5 text-sm"
                          >
                            <Footprints size={16} /> {joiningId === w.sessionId ? 'Dołączam…' : 'Dołącz'}
                          </PrimaryButton>
                        </Card>
                      </motion.div>
                    )
                  })}
                </div>
              )}
            </section>

            {/* ── Feed: ostatnio w społeczności ── */}
            {feed.length > 0 && (
              <section>
                <h2 className="mb-3 flex items-center gap-2 font-display text-lg font-bold text-ink">
                  <Heart size={18} className="text-leaf" /> Ostatnio w społeczności
                </h2>
                <div className="space-y-3">
                  {feed.slice(0, 15).map((r, i) => {
                    const isCleanup = r.kind === 'cleanup' || r.status === 'cleaned'
                    const beforeAfter = r.photoBeforeUrl && r.photoAfterUrl
                    const singlePhoto = !beforeAfter ? r.photoAfterUrl || r.photoUrl || r.photoBeforeUrl : null
                    const panelOpen = openComments.has(r.id)
                    const list = comments[r.id] ?? []
                    return (
                      <motion.div
                        key={r.id}
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ delay: Math.min(i, 5) * 0.05 }}
                      >
                        <Card className="overflow-hidden">
                          {/* nagłówek wpisu */}
                          <div className="flex items-center gap-3 p-4 pb-3">
                            <Avatar name={r.author ?? '?'} size={40} />
                            <div className="min-w-0 flex-1">
                              <div className="text-[15px] font-bold leading-tight text-ink [overflow-wrap:anywhere]">
                                {r.author ?? 'Ktoś'}
                              </div>
                              <div className="text-xs font-semibold text-muted">
                                {isCleanup ? 'posprzątał(a)' : 'zgłosił(a) problem'}
                                {r.type ? ` • ${r.type}` : ''}
                                {r.createdAt ? ` • ${timeAgo(r.createdAt)}` : ''}
                              </div>
                            </div>
                            <Pill tone={isCleanup ? 'leaf' : 'sand'}>{isCleanup ? '+25 pkt' : 'zgłoszone'}</Pill>
                          </div>

                          {/* treść */}
                          {r.description && (
                            <p className="px-4 pb-3 text-sm leading-snug text-ink [overflow-wrap:anywhere]">
                              {r.description}
                            </p>
                          )}

                          {/* zdjęcia — duże, widoczne dla wszystkich */}
                          {beforeAfter ? (
                            <div className="grid grid-cols-2 gap-0.5">
                              <div className="relative">
                                <img src={r.photoBeforeUrl!} alt="Przed sprzątaniem" className="h-44 w-full object-cover" loading="lazy" />
                                <span className="absolute left-2 top-2 rounded-full bg-black/50 px-2 py-0.5 text-[10px] font-bold text-white">PRZED</span>
                              </div>
                              <div className="relative">
                                <img src={r.photoAfterUrl!} alt="Po sprzątaniu" className="h-44 w-full object-cover" loading="lazy" />
                                <span className="absolute left-2 top-2 rounded-full bg-leaf/90 px-2 py-0.5 text-[10px] font-bold text-white">PO</span>
                              </div>
                            </div>
                          ) : singlePhoto ? (
                            <img src={singlePhoto} alt="" className="max-h-80 w-full object-cover" loading="lazy" />
                          ) : null}

                          {/* akcje: polub + komentuj */}
                          <div className="flex items-center gap-4 px-4 py-3">
                            <button
                              type="button"
                              onClick={() => toggleLike(r.id)}
                              className={`inline-flex items-center gap-1.5 text-sm font-bold transition active:scale-90 ${
                                r.likedByMe ? 'text-rose-500' : 'text-muted'
                              }`}
                            >
                              <Heart size={20} weight={r.likedByMe ? 'fill' : 'regular'} />
                              {(r.likeCount ?? 0) > 0 && r.likeCount}
                            </button>
                            <button
                              type="button"
                              onClick={() => toggleCommentsPanel(r.id)}
                              className={`inline-flex items-center gap-1.5 text-sm font-bold transition active:scale-90 ${
                                panelOpen ? 'text-sea' : 'text-muted'
                              }`}
                            >
                              <ChatCircle size={20} />
                              {(r.commentCount ?? 0) > 0 ? r.commentCount : 'Komentuj'}
                            </button>
                          </div>

                          {/* komentarze */}
                          {panelOpen && (
                            <div className="border-t border-[rgba(20,52,58,0.06)] px-4 py-3">
                              {list.length > 0 && (
                                <div className="mb-3 space-y-2">
                                  {list.map((c) => (
                                    <div key={c.id} className="flex items-start gap-2">
                                      <Avatar name={c.author} size={26} />
                                      <p className="min-w-0 flex-1 text-sm leading-snug text-ink [overflow-wrap:anywhere]">
                                        <span className="font-bold">{c.author}</span>{' '}
                                        <span>{c.body}</span>{' '}
                                        <span className="text-[11px] font-semibold text-muted">{timeAgo(c.createdAt)}</span>
                                      </p>
                                    </div>
                                  ))}
                                </div>
                              )}
                              <div className="flex items-center gap-2">
                                <input
                                  value={commentDraft[r.id] ?? ''}
                                  onChange={(e) => setCommentDraft((d) => ({ ...d, [r.id]: e.target.value.slice(0, 500) }))}
                                  onKeyDown={(e) => e.key === 'Enter' && sendComment(r.id)}
                                  placeholder="Dodaj komentarz…"
                                  className="min-w-0 flex-1 rounded-2xl border border-white/70 bg-white/70 px-3.5 py-2 text-sm text-ink outline-none placeholder:text-muted/70 focus:ring-2 focus:ring-sea/30"
                                />
                                <button
                                  type="button"
                                  onClick={() => sendComment(r.id)}
                                  disabled={commentSending === r.id || !(commentDraft[r.id] ?? '').trim()}
                                  className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-gradient-to-br from-sea to-deep text-white transition active:scale-90 disabled:opacity-50"
                                  aria-label="Wyślij komentarz"
                                >
                                  <PaperPlaneRight size={16} weight="fill" />
                                </button>
                              </div>
                            </div>
                          )}
                        </Card>
                      </motion.div>
                    )
                  })}
                </div>
              </section>
            )}

            {feed.length === 0 && (
              <Card className="p-4">
                <p className="text-sm text-muted">
                  Na tablicy na razie pusto. Pochwal się sprzątaniem na ekranie Eko — Twój wpis pojawi się tutaj 🌊
                </p>
              </Card>
            )}
            </>
            )}

            {tab === 'friends' && (
            <>
            {/* ── Wiadomości ── */}
            {conversations.length > 0 && (
              <section>
                <h2 className="mb-3 flex items-center gap-2 font-display text-lg font-bold text-ink">
                  <ChatCircle size={18} className="text-sea" /> Wiadomości
                </h2>
                <Card className="divide-y divide-[rgba(20,52,58,0.06)] p-2">
                  {conversations.map((c) => (
                    <button
                      key={c.userId}
                      type="button"
                      onClick={() => navigate(`/chat/${c.userId}`, { state: { name: c.name, avatar: c.avatar } })}
                      className="flex w-full items-center gap-3 rounded-2xl px-3 py-2.5 text-left transition active:scale-[0.99]"
                    >
                      <Avatar name={c.name} size={44} />
                      <div className="min-w-0 flex-1">
                        <div className="font-display text-base font-bold text-ink">{c.name}</div>
                        <p className="truncate text-xs text-muted">
                          {c.lastFromMe ? 'Ty: ' : ''}
                          {c.lastBody}
                        </p>
                      </div>
                      {c.unread > 0 && (
                        <span className="grid h-6 min-w-6 shrink-0 place-items-center rounded-full bg-sea px-1.5 text-xs font-bold text-white">
                          {c.unread}
                        </span>
                      )}
                    </button>
                  ))}
                </Card>
              </section>
            )}

            {/* ── Znajomi ── */}
            <section>
              <h2 className="mb-3 flex items-center gap-2 font-display text-lg font-bold text-ink">
                <UsersThree size={18} className="text-sea" /> Znajomi
              </h2>

              {friends.incoming.length > 0 && (
                <div className="mb-3 space-y-2.5">
                  {friends.incoming.map((req) => (
                    <Card key={req.requestId} className="p-3">
                      <div className="flex items-center gap-3">
                        <Avatar name={req.user.name} size={40} />
                        <span className="flex-1 text-sm font-bold text-ink">{req.user.name} chce się zaprzyjaźnić</span>
                      </div>
                      <div className="mt-2.5 grid grid-cols-2 gap-2">
                        <SoftButton
                          onClick={() => respond(req.requestId, false)}
                          disabled={respondingId === req.requestId}
                          className="py-2 text-sm"
                        >
                          Odrzuć
                        </SoftButton>
                        <PrimaryButton
                          onClick={() => respond(req.requestId, true)}
                          disabled={respondingId === req.requestId}
                          className="py-2 text-sm"
                        >
                          Przyjmij
                        </PrimaryButton>
                      </div>
                    </Card>
                  ))}
                </div>
              )}

              {friends.accepted.length > 0 ? (
                <div className="space-y-2.5">
                  {friends.accepted.map((f) => {
                    const shared = f.interests.filter((t) => myInterests.includes(t))
                    return (
                      <Card key={f.id} className="p-3">
                        <div className="flex items-center gap-3">
                          <Avatar name={f.name} size={44} />
                          <div className="min-w-0 flex-1">
                            <div className="font-display text-base font-bold text-ink">{f.name}</div>
                            {shared.length > 0 && (
                              <div className="mt-1 flex flex-wrap gap-1">
                                {shared.map((s) => (
                                  <Pill key={s} tone="leaf">
                                    {s}
                                  </Pill>
                                ))}
                              </div>
                            )}
                          </div>
                          <SoftButton
                            onClick={() => navigate(`/chat/${f.id}`, { state: { name: f.name, avatar: f.avatar } })}
                            className="px-4 py-2 text-sm"
                          >
                            <ChatCircle size={16} /> Napisz
                          </SoftButton>
                          <button
                            type="button"
                            onClick={() => unfriend(f.id)}
                            disabled={unfriendingId === f.id}
                            aria-label={`Usuń ${f.name} ze znajomych`}
                            className={`shrink-0 rounded-full px-2.5 py-2 text-xs font-bold transition active:scale-95 disabled:opacity-50 ${
                              unfriendArmedId === f.id ? 'bg-rose-500/15 text-rose-600' : 'text-muted'
                            }`}
                          >
                            {unfriendArmedId === f.id ? 'Na pewno?' : <X size={15} />}
                          </button>
                          <button
                            type="button"
                            onClick={() => block(f.id)}
                            disabled={blockingId === f.id}
                            aria-label={`Zablokuj ${f.name}`}
                            title="Zablokuj — kończy znajomość i zamyka czat, zaproszenia i wspólne spacery"
                            className={`shrink-0 rounded-full px-2.5 py-2 text-xs font-bold transition active:scale-95 disabled:opacity-50 ${
                              blockArmedId === f.id ? 'bg-rose-500/15 text-rose-600' : 'text-muted'
                            }`}
                          >
                            {blockArmedId === f.id ? 'Zablokować?' : <Prohibit size={15} />}
                          </button>
                        </div>
                      </Card>
                    )
                  })}
                </div>
              ) : friends.incoming.length === 0 ? (
                <Card className="p-4">
                  <p className="text-sm text-muted">Nie masz jeszcze znajomych — poszukaj kogoś poniżej i zaproś na spacer.</p>
                </Card>
              ) : null}

              {friends.outgoing.length > 0 && (
                <div className="mt-2.5 space-y-1.5">
                  {friends.outgoing.map((req) => (
                    <div key={req.requestId} className="flex items-center gap-3 rounded-2xl px-3 py-2 text-sm">
                      <Avatar name={req.user.name} size={32} />
                      <span className="flex-1 font-semibold text-muted">{req.user.name}</span>
                      <Pill tone="muted">wysłano</Pill>
                    </div>
                  ))}
                </div>
              )}
            </section>

            {/* ── Znajdź ludzi ── */}
            <section>
              <h2 className="mb-3 flex items-center gap-2 font-display text-lg font-bold text-ink">
                <MagnifyingGlass size={18} className="text-sea" /> Znajdź ludzi
              </h2>
              <Card className="p-3">
                <input
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder="Szukaj po imieniu…"
                  className="w-full rounded-xl border border-white/70 bg-white/80 px-3 py-2.5 text-sm outline-none"
                />
                {searching && <p className="mt-2 px-1 text-xs text-muted">Szukam…</p>}
                {!searching && query.trim().length >= 2 && visibleResults.length === 0 && (
                  <p className="mt-2 px-1 text-xs text-muted">Nikogo nie znaleziono.</p>
                )}
                {visibleResults.length > 0 && (
                  <div className="mt-2 divide-y divide-[rgba(20,52,58,0.06)]">
                    {visibleResults.map((r) => {
                      const sent = sentIds.has(r.id)
                      const conflict = conflictIds.has(r.id)
                      return (
                        <div key={r.id} className="flex items-center gap-3 py-2.5">
                          <Avatar name={r.name} size={40} />
                          <span className="flex-1 text-sm font-bold text-ink">{r.name}</span>
                          {sent ? (
                            <Pill tone="muted">wysłano</Pill>
                          ) : conflict ? (
                            <Pill tone="muted">już się znacie</Pill>
                          ) : (
                            <SoftButton onClick={() => addFriend(r.id)} disabled={sendingId === r.id} className="px-4 py-2 text-sm">
                              <UserPlus size={16} /> Dodaj
                            </SoftButton>
                          )}
                        </div>
                      )
                    })}
                  </div>
                )}
              </Card>
            </section>

            {/* ── Dopasowani do Ciebie ── */}
            <section>
              <h2 className="mb-3 flex items-center gap-2 font-display text-lg font-bold text-ink">
                <Heart size={18} className="text-leaf" /> Dopasowani do Ciebie
              </h2>
              <div className="space-y-3">
                {matches.map((m, i) => {
                  const shared = m.interests.filter((t) => myInterests.includes(t))
                  return (
                    <motion.div key={m.id} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.07 }}>
                      <Card className="p-4">
                        <div className="flex items-center gap-3">
                          <Avatar name={m.name} size={48} />
                          <div className="flex-1">
                            <div className="flex items-center gap-2">
                              <span className="font-display text-lg font-bold text-ink">{m.name}</span>
                              <span className="inline-flex items-center gap-0.5 text-xs font-bold text-muted">
                                <MapPin size={12} /> {m.distance}
                              </span>
                            </div>
                            <p className="text-xs text-muted">{m.bio}</p>
                          </div>
                          <SoonBadge />
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
                          <SoftButton disabled className="py-2.5 text-sm">
                            <ChatCircle size={16} /> Wkrótce
                          </SoftButton>
                          <PrimaryButton disabled className="py-2.5 text-sm">
                            <Footprints size={16} /> Wkrótce
                          </PrimaryButton>
                        </div>
                      </Card>
                    </motion.div>
                  )
                })}
              </div>
            </section>
            </>
            )}
          </>
        )}

        {/* ── Spacery / wspólne wyjścia (zapowiedź; solo: na dole zakładki Feed) ── */}
        {(isTeam || tab === 'feed') && (
        <section>
          <h2 className="mb-3 font-display text-lg font-bold text-ink">{isTeam ? 'Spacery działów' : 'Wspólne spacery'}</h2>
          <div className="space-y-3">
            {walks.map((w, i) => (
              <motion.div key={w.id} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.06 }}>
                <Card className="p-4">
                  <div className="flex items-center gap-3">
                    <Avatar name={w.who} size={48} />
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
                    <SoonBadge />
                  </div>
                  <p className="mt-2.5 text-sm text-muted">„{w.vibe}"</p>
                  <div className="mt-3 grid grid-cols-2 gap-2.5">
                    <SoftButton disabled className="py-2.5 text-sm">
                      Wkrótce
                    </SoftButton>
                    <PrimaryButton disabled className="py-2.5 text-sm">
                      Wkrótce
                    </PrimaryButton>
                  </div>
                </Card>
              </motion.div>
            ))}
          </div>
        </section>
        )}

        {/* ── Ranking ── */}
        {(isTeam || tab === 'ranking') && (
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
                    <Avatar name={r.name} size={36} />
                    <span className={`flex-1 text-sm font-bold ${r.isMe ? 'text-deep' : 'text-ink'}`}>
                      {r.name} {r.isMe && <Pill tone="sea">to Ty</Pill>} {r.isDemo && <Pill tone="muted">demo</Pill>}
                    </span>
                    <span className="font-display text-base font-bold text-sea">{r.points}</span>
                  </div>
                ))}
          </Card>
          {!isTeam && board.some((r) => r.isDemo) && (
            <p className="mt-2 px-1 text-xs text-muted">
              Konta z plakietką „demo" pokazują przykładowe dane — reszta rankingu jest prawdziwa.
            </p>
          )}
        </section>
        )}
      </div>
    </div>
  )
}
