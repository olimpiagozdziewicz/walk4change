import { useEffect, useState } from 'react'
import QRCode from 'qrcode'
import { joinUrl } from '../lib/activeWalk'

/**
 * QR „dołącz do mojego spaceru” (spec 2026-10-05). Druga osoba skanuje go
 * zwykłym aparatem telefonu — z apką otwiera się apka (App Link), bez apki PWA.
 * Ten sam kod może zeskanować kilka osób (limit serwera: 10 uczestników).
 */
export function JoinQr({ code }: { code: string }) {
  const [src, setSrc] = useState<string | null>(null)
  useEffect(() => {
    let alive = true
    QRCode.toDataURL(joinUrl(code), { margin: 1, width: 360, color: { dark: '#0b4f5c', light: '#ffffff' } })
      .then((url) => { if (alive) setSrc(url) })
      .catch(() => { if (alive) setSrc(null) })
    return () => { alive = false }
  }, [code])
  if (!src) return null
  return (
    <img
      src={src}
      alt={`Kod QR do dołączenia do spaceru (kod ${code})`}
      className="mx-auto mt-3 aspect-square w-full max-w-[220px] rounded-2xl bg-white p-2"
    />
  )
}
