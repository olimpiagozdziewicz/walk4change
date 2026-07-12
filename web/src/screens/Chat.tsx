import { useEffect, useRef, useState } from 'react'
import { useNavigate, useParams, useLocation } from 'react-router-dom'
import { CaretLeft, PaperPlaneRight, Prohibit } from '@phosphor-icons/react'
import { Avatar } from '../components/Avatar'
import { currentUserId } from '../lib/auth'
import { ApiError } from '../lib/http'
import { api, type ChatMessage } from '../lib/api'

const POLL_MS = 5000
const MAX_LEN = 2000
const NEAR_BOTTOM_PX = 120

function fmtTime(iso: string): string {
  try {
    return new Date(iso).toLocaleTimeString('pl-PL', { hour: '2-digit', minute: '2-digit' })
  } catch {
    return ''
  }
}

interface ChatLocationState {
  name?: string
  avatar?: string
}

export function Chat() {
  const navigate = useNavigate()
  const location = useLocation()
  const { userId } = useParams<{ userId: string }>()
  const state = (location.state as ChatLocationState | null) ?? null

  const [partnerName, setPartnerName] = useState<string | null>(state?.name ?? null)
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [loadError, setLoadError] = useState<string | null>(null)
  const [sendError, setSendError] = useState<string | null>(null)
  const [input, setInput] = useState('')
  const [sending, setSending] = useState(false)
  // blokada rozmówcy: pierwszy klik uzbraja, drugi blokuje i wraca do listy
  const [blockArmed, setBlockArmed] = useState(false)
  const [blocking, setBlocking] = useState(false)

  const myId = currentUserId()
  const seenIdsRef = useRef<Set<string>>(new Set())
  const lastCreatedAtRef = useRef<string | null>(null)
  const nearBottomRef = useRef(true)
  const bottomRef = useRef<HTMLDivElement | null>(null)

  const displayName = partnerName ?? 'Rozmowa'

  // fallback: brak imienia z location.state — spróbuj znaleźć w liście znajomych
  useEffect(() => {
    if (partnerName || !userId) return
    api
      .getFriends()
      .then((data) => {
        const found = data.accepted.find((f) => f.id === userId)
        if (found) setPartnerName(found.name)
      })
      .catch(() => {})
  }, [partnerName, userId])

  // pierwsze wczytanie historii
  useEffect(() => {
    if (!userId) return
    let cancelled = false
    setMessages([])
    seenIdsRef.current = new Set()
    lastCreatedAtRef.current = null
    setLoadError(null)
    api
      .getMessages(userId)
      .then((msgs) => {
        if (cancelled) return
        msgs.forEach((m) => seenIdsRef.current.add(m.id))
        if (msgs.length) lastCreatedAtRef.current = msgs[msgs.length - 1].createdAt
        nearBottomRef.current = true
        setMessages(msgs)
      })
      .catch((err) => {
        if (cancelled) return
        setLoadError(
          err instanceof ApiError && err.status === 403
            ? 'Możecie pisać tylko ze znajomymi.'
            : 'Nie udało się wczytać rozmowy.',
        )
      })
    return () => {
      cancelled = true
    }
  }, [userId])

  // polling nowych wiadomości
  useEffect(() => {
    if (!userId) return
    const id = window.setInterval(() => {
      api
        .getMessages(userId, lastCreatedAtRef.current ?? undefined)
        .then((msgs) => {
          if (!msgs.length) return
          const fresh = msgs.filter((m) => !seenIdsRef.current.has(m.id))
          if (!fresh.length) return
          fresh.forEach((m) => seenIdsRef.current.add(m.id))
          lastCreatedAtRef.current = msgs[msgs.length - 1].createdAt
          setMessages((prev) => [...prev, ...fresh])
        })
        .catch(() => {})
    }, POLL_MS)
    return () => window.clearInterval(id)
  }, [userId])

  // śledzenie, czy user jest blisko dołu strony (żeby nie szarpać scrolla)
  useEffect(() => {
    const onScroll = () => {
      const distance = document.documentElement.scrollHeight - (window.scrollY + window.innerHeight)
      nearBottomRef.current = distance < NEAR_BOTTOM_PX
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    onScroll()
    return () => window.removeEventListener('scroll', onScroll)
  }, [])

  useEffect(() => {
    if (nearBottomRef.current) {
      bottomRef.current?.scrollIntoView({ block: 'end' })
    }
  }, [messages])

  const send = async () => {
    const body = input.trim().slice(0, MAX_LEN)
    if (!body || !userId || sending) return
    setSending(true)
    setSendError(null)
    try {
      const msg = await api.sendMessage(userId, body)
      if (msg) {
        if (!seenIdsRef.current.has(msg.id)) {
          seenIdsRef.current.add(msg.id)
          lastCreatedAtRef.current = msg.createdAt
          nearBottomRef.current = true
          setMessages((prev) => [...prev, msg])
        }
        setInput('')
      }
    } catch (err) {
      if (err instanceof ApiError && err.status === 403) {
        setSendError('Możecie pisać tylko ze znajomymi.')
      } else {
        setSendError('Nie udało się wysłać wiadomości.')
      }
    } finally {
      setSending(false)
    }
  }

  const blockPartner = async () => {
    if (!userId || blocking) return
    if (!blockArmed) {
      setBlockArmed(true)
      window.setTimeout(() => setBlockArmed(false), 4000)
      return
    }
    setBlocking(true)
    try {
      await api.blockUser(userId)
      navigate('/community')
    } catch {
      setSendError('Nie udało się zablokować — spróbuj ponownie.')
    } finally {
      setBlocking(false)
      setBlockArmed(false)
    }
  }

  if (!userId) {
    return (
      <div className="px-5 pt-6">
        <p className="text-sm text-muted">Nie znaleziono rozmowy.</p>
      </div>
    )
  }

  return (
    <div className="flex flex-col">
      <header className="sticky top-0 z-20 flex items-center gap-3 bg-bg/85 px-5 pb-3 pt-4 backdrop-blur-md">
        <button
          type="button"
          onClick={() => navigate('/community')}
          aria-label="Wróć"
          className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-full glass text-deep transition active:scale-95"
        >
          <CaretLeft size={20} />
        </button>
        <Avatar name={displayName} size={40} />
        <div className="min-w-0 flex-1">
          <div className="truncate font-display text-lg font-bold text-ink">{displayName}</div>
        </div>
        <button
          type="button"
          onClick={blockPartner}
          disabled={blocking}
          aria-label={`Zablokuj ${displayName}`}
          title="Zablokuj — kończy znajomość i zamyka ten czat na stałe"
          className={`inline-flex h-10 shrink-0 items-center justify-center gap-1 rounded-full px-3 text-xs font-bold transition active:scale-95 disabled:opacity-50 ${
            blockArmed ? 'bg-rose-500/15 text-rose-600' : 'glass text-muted'
          }`}
        >
          <Prohibit size={16} /> {blockArmed ? 'Zablokować?' : ''}
        </button>
      </header>

      <div className="min-w-0 px-5 pb-4 pt-1">
        {loadError && <p className="mb-3 text-sm font-semibold text-rose-600">{loadError}</p>}
        {!loadError && messages.length === 0 && (
          <p className="mt-6 text-center text-sm text-muted">Brak wiadomości — napisz coś pierwszy/pierwsza!</p>
        )}
        <div className="space-y-3">
          {messages.map((m) => {
            const mine = m.senderId === myId
            return (
              <div key={m.id} className={`flex min-w-0 flex-col ${mine ? 'items-end' : 'items-start'}`}>
                <div
                  className={`min-w-0 max-w-[78%] rounded-2xl px-4 py-2.5 text-sm ${
                    mine ? 'bg-gradient-to-br from-sea to-deep text-white' : 'glass text-ink'
                  }`}
                  style={{ overflowWrap: 'anywhere' }}
                >
                  <p className="whitespace-pre-wrap break-words">{m.body}</p>
                </div>
                <span className="mt-1 px-1 text-[11px] font-semibold text-muted">{fmtTime(m.createdAt)}</span>
              </div>
            )
          })}
        </div>
        <div ref={bottomRef} />
      </div>

      <div className="sticky bottom-[calc(env(safe-area-inset-bottom,0px)+80px)] z-20 mt-2 px-5 lg:bottom-3">
        <div className="glass rounded-[var(--radius-card)] p-2 shadow-[0_18px_40px_rgba(12,90,113,0.10)]">
          {sendError && <p className="mb-1.5 px-2 text-xs font-semibold text-rose-600">{sendError}</p>}
          <div className="flex items-center gap-2">
            <input
              value={input}
              onChange={(e) => setInput(e.target.value.slice(0, MAX_LEN))}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault()
                  send()
                }
              }}
              maxLength={MAX_LEN}
              placeholder="Napisz coś…"
              className="min-w-0 flex-1 rounded-xl border border-white/70 bg-white/80 px-3 py-2.5 text-sm outline-none"
            />
            <button
              type="button"
              onClick={send}
              disabled={!input.trim() || sending}
              aria-label="Wyślij"
              className="grid h-10 w-10 shrink-0 place-items-center rounded-full bg-gradient-to-br from-sea to-deep text-white transition active:scale-95 disabled:opacity-50"
            >
              <PaperPlaneRight size={18} weight="fill" />
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
