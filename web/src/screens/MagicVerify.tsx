import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { exchangeSupabaseSession } from '../lib/auth'

/**
 * Magic-link landing (/auth/magic). Supabase establishes a session from the URL
 * (implicit flow); we retry-exchange it for the backend JWT, then enter the app.
 */
export function MagicVerify() {
  const nav = useNavigate()
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let done = false
    const attempt = async (n: number) => {
      if (done) return
      try {
        if (await exchangeSupabaseSession()) {
          done = true
          window.history.replaceState(null, '', '/walk')
          nav('/walk')
          return
        }
      } catch {
        /* retry below */
      }
      if (n < 9) window.setTimeout(() => attempt(n + 1), 400)
      else setError('Nie udało się zalogować linkiem. Poproś o nowy magiczny link.')
    }
    attempt(0)
    return () => { done = true }
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
