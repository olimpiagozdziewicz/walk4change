import { useEffect, useRef, useState } from 'react'
import { confirmEmailVerification } from '../lib/auth'

type Status = 'working' | 'ok' | 'error'

/**
 * Landing linku weryfikacyjnego (/auth/verify-email?token=…).
 * Potwierdza skrzynkę — celowo NIE loguje (mail weryfikacyjny nie jest
 * poświadczeniem logowania; spec 2026-07-13).
 */
export function VerifyEmail() {
  const [status, setStatus] = useState<Status>('working')
  const ran = useRef(false)

  useEffect(() => {
    if (ran.current) return
    ran.current = true
    const token = new URLSearchParams(window.location.search).get('token')
    if (!token) { setStatus('error'); return }
    confirmEmailVerification(token)
      .then(() => setStatus('ok'))
      .catch(() => setStatus('error'))
  }, [])

  return (
    <div className="grid min-h-[60svh] place-items-center px-6 text-center">
      {status === 'working' && <p className="text-muted">Potwierdzanie e-maila…</p>}
      {status === 'ok' && (
        <div>
          <p className="font-display text-xl font-bold text-deep">✓ E-mail potwierdzony</p>
          <p className="mt-2 text-sm text-muted">Otwarte spacery są odblokowane. Możesz wrócić do aplikacji.</p>
          <a href={import.meta.env.BASE_URL || '/'} className="mt-4 inline-block font-bold text-sea">Przejdź do SeaSteps →</a>
        </div>
      )}
      {status === 'error' && (
        <div>
          <p className="font-semibold text-rose-600">Link jest nieprawidłowy albo wygasł (ważny 24 h).</p>
          <p className="mt-2 text-sm text-muted">Nowy link wyślesz z Profilu w aplikacji.</p>
          <a href={import.meta.env.BASE_URL || '/'} className="mt-3 inline-block font-bold text-sea">← Wróć do SeaSteps</a>
        </div>
      )}
    </div>
  )
}
