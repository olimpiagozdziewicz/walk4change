import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { apiRequest, setToken } from '../lib/http'
import { setAuthed, setCurrentUserId } from '../lib/auth'

/** Consumes a magic-link token (/auth/magic?token=…), stores the JWT, enters the app. */
export function MagicVerify() {
  const nav = useNavigate()
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const token = new URLSearchParams(window.location.search).get('token')
    if (!token) { setError('Brak tokenu w linku.'); return }
    let cancelled = false
    ;(async () => {
      try {
        const res = await apiRequest<{ id?: string }>('/auth/magic/verify', { method: 'POST', auth: false, body: { token } })
        if (cancelled) return
        if (res.token) setToken(res.token)
        if (res.data?.id) setCurrentUserId(res.data.id)
        setAuthed(true)
        window.history.replaceState(null, '', '/walk')
        nav('/walk')
      } catch {
        if (!cancelled) setError('Link wygasł lub jest nieprawidłowy. Poproś o nowy magiczny link.')
      }
    })()
    return () => { cancelled = true }
  }, [nav])

  return (
    <div className="grid min-h-[60svh] place-items-center px-6 text-center">
      {error ? (
        <div>
          <p className="font-semibold text-rose-600">{error}</p>
          <a href="/" className="mt-3 inline-block font-bold text-sea">← Wróć</a>
        </div>
      ) : (
        <p className="text-muted">Logowanie magicznym linkiem…</p>
      )}
    </div>
  )
}
