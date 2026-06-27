import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Envelope, ArrowRight, CheckCircle, Sparkle } from '@phosphor-icons/react'
import { setAuthed } from '../lib/auth'

/** Logowanie magic-linkiem (bez hasła). Demo: symuluje wysyłkę i wpuszcza do apki. */
export function MagicLinkForm() {
  const nav = useNavigate()
  const [email, setEmail] = useState('')
  const [sent, setSent] = useState(false)

  if (sent) {
    return (
      <div className="rounded-2xl bg-white/85 p-4 text-left ring-1 ring-white/60">
        <div className="flex items-center gap-2 font-bold text-deep">
          <CheckCircle size={20} weight="fill" className="text-leaf" /> Sprawdź skrzynkę!
        </div>
        <p className="mt-1 text-sm text-muted">
          Wysłaliśmy magiczny link na <b className="text-ink">{email}</b>. Kliknij go, żeby wejść — bez hasła.
        </p>
        <button
          onClick={() => {
            setAuthed(true)
            nav('/')
          }}
          className="mt-3 inline-flex items-center gap-2 rounded-xl bg-gradient-to-br from-sea to-deep px-4 py-2.5 text-sm font-bold text-white transition active:scale-95"
        >
          Otwórz link (demo) <ArrowRight size={16} />
        </button>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-2 sm:flex-row">
      <div className="flex flex-1 items-center gap-2 rounded-2xl bg-white/90 px-4 py-3 ring-1 ring-white/60">
        <Envelope size={18} className="text-muted" />
        <input
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          type="email"
          placeholder="twój@email.pl"
          className="w-full bg-transparent text-sm text-ink outline-none placeholder:text-muted/70"
        />
      </div>
      <button
        onClick={() => email.includes('@') && setSent(true)}
        className="inline-flex items-center justify-center gap-2 rounded-2xl bg-gradient-to-br from-sea to-deep px-5 py-3 text-sm font-bold text-white shadow-[0_14px_28px_rgba(12,90,113,0.25)] transition active:scale-95"
      >
        <Sparkle size={16} weight="fill" /> Wyślij magiczny link
      </button>
    </div>
  )
}
