import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Envelope, Lock, ArrowRight, Footprints, Leaf, UsersThree, Warning } from '@phosphor-icons/react'
import { Logo } from '../components/Logo'
import { FootstepTrail } from '../components/Footsteps'
import { login, register, requestMagicLink } from '../lib/auth'

type Tab = 'login' | 'signup'

export function Login() {
  const nav = useNavigate()
  const [tab, setTab] = useState<Tab>('login')
  const [email, setEmail] = useState('')
  const [pass, setPass] = useState('')
  const [pass2, setPass2] = useState('')
  const [error, setError] = useState<string | null>(() =>
    new URLSearchParams(window.location.search).has('expired')
      ? 'Sesja wygasła — zaloguj się ponownie.'
      : null,
  )
  const [loading, setLoading] = useState(false)
  const [magicMsg, setMagicMsg] = useState<string | null>(null)
  const [terms, setTerms] = useState(false)

  const submit = async () => {
    setError(null)
    setMagicMsg(null)
    if (!email || !pass) { setError('Podaj e-mail i hasło.'); return }
    if (tab === 'signup' && pass !== pass2) { setError('Hasła się nie zgadzają.'); return }
    if (tab === 'signup' && !terms) { setError('Zaakceptuj regulamin i politykę prywatności, aby założyć konto.'); return }
    setLoading(true)
    try {
      if (tab === 'login') {
        await login(email, pass)
      } else {
        await register(email, pass, email.split('@')[0], terms)
      }
      nav('/')
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Błąd logowania.')
    } finally {
      setLoading(false)
    }
  }

  const sendMagicLink = async () => {
    if (loading) return
    if (!email.includes('@')) { setError('Podaj e-mail, aby wysłać magiczny link.'); return }
    setError(null)
    setMagicMsg(null)
    setLoading(true)
    try {
      await requestMagicLink(email)
      setMagicMsg(`✓ Sprawdź skrzynkę — magiczny link poszedł na ${email.trim()}.`)
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Nie udało się wysłać magicznego linku.')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="flex min-h-[100svh] flex-col lg:flex-row">
      {/* brand panel */}
      <div className="relative flex flex-col justify-center overflow-hidden bg-gradient-to-br from-sea to-deep px-6 py-10 text-white lg:w-1/2 lg:px-16 lg:py-0">
        <div className="pointer-events-none absolute -bottom-20 -right-10 h-72 w-72 rounded-full bg-white/10 blur-3xl" />
        <div className="relative">
          <div className="[&_*]:!text-white">
            <Logo />
          </div>
          <h1 className="mt-6 font-display text-3xl font-bold leading-tight tracking-tight lg:mt-10 lg:text-5xl">
            Spacer, który robi dobrze Tobie i naturze
          </h1>
          <p className="mt-3 max-w-md text-white/85 lg:mt-5 lg:text-lg">
            Ruch, natura, ludzie i małe działania dla środowiska — z lekkością.
          </p>
          <div className="mt-6 hidden gap-5 lg:flex">
            <span className="inline-flex items-center gap-2 text-sm font-semibold text-white/90"><Footprints size={18} weight="fill" /> Kroki = punkty</span>
            <span className="inline-flex items-center gap-2 text-sm font-semibold text-white/90"><Leaf size={18} weight="fill" /> Natura ×3</span>
            <span className="inline-flex items-center gap-2 text-sm font-semibold text-white/90"><UsersThree size={18} weight="fill" /> Razem</span>
          </div>
        </div>
      </div>

      {/* formularz */}
      <div className="relative flex flex-1 items-center justify-center px-6 py-10 lg:px-16">
        <div className="pointer-events-none absolute right-6 top-6 opacity-30">
          <FootstepTrail count={4} color="#0f8b8d" />
        </div>
        <div className="w-full max-w-sm">
          {/* zakładki */}
          <div className="mb-6 inline-flex w-full rounded-2xl bg-sea/8 p-1 text-sm font-bold">
            <button
              onClick={() => { setTab('login'); setError(null); setMagicMsg(null) }}
              className={`flex-1 rounded-xl py-2.5 transition ${tab === 'login' ? 'bg-white text-deep shadow' : 'text-muted'}`}
            >
              Zaloguj się
            </button>
            <button
              onClick={() => { setTab('signup'); setError(null); setMagicMsg(null) }}
              className={`flex-1 rounded-xl py-2.5 transition ${tab === 'signup' ? 'bg-white text-deep shadow' : 'text-muted'}`}
            >
              Załóż konto
            </button>
          </div>

          <h2 className="font-display text-2xl font-bold text-ink">
            {tab === 'login' ? 'Witaj z powrotem' : 'Dołącz do SeaSteps'}
          </h2>
          <p className="mt-1 text-sm text-muted">
            {tab === 'login' ? 'Zaloguj się i ruszaj nad wodę.' : 'Załóż konto i zrób pierwszy krok.'}
          </p>

          <label className="mt-6 block text-xs font-bold uppercase tracking-wide text-muted">E-mail</label>
          <div className="mt-1.5 flex items-center gap-2 rounded-2xl border border-white/70 bg-white/70 px-4 py-3 focus-within:ring-2 focus-within:ring-sea/30">
            <Envelope size={18} className="text-muted" />
            <input value={email} onChange={(e) => setEmail(e.target.value)} type="email" placeholder="twój@email.pl" className="w-full bg-transparent text-sm text-ink outline-none placeholder:text-muted/70" />
          </div>

          <label className="mt-4 block text-xs font-bold uppercase tracking-wide text-muted">Hasło</label>
          <div className="mt-1.5 flex items-center gap-2 rounded-2xl border border-white/70 bg-white/70 px-4 py-3 focus-within:ring-2 focus-within:ring-sea/30">
            <Lock size={18} className="text-muted" />
            <input value={pass} onChange={(e) => setPass(e.target.value)} type="password" placeholder="••••••••" className="w-full bg-transparent text-sm text-ink outline-none placeholder:text-muted/70" />
          </div>

          {tab === 'signup' && (
            <>
              <label className="mt-4 block text-xs font-bold uppercase tracking-wide text-muted">Powtórz hasło</label>
              <div className="mt-1.5 flex items-center gap-2 rounded-2xl border border-white/70 bg-white/70 px-4 py-3 focus-within:ring-2 focus-within:ring-sea/30">
                <Lock size={18} className="text-muted" />
                <input value={pass2} onChange={(e) => setPass2(e.target.value)} type="password" placeholder="••••••••" className="w-full bg-transparent text-sm text-ink outline-none placeholder:text-muted/70" />
              </div>
              <label className="mt-4 flex cursor-pointer items-start gap-2.5 text-xs text-muted">
                <input
                  type="checkbox"
                  checked={terms}
                  onChange={(e) => setTerms(e.target.checked)}
                  className="mt-0.5 h-4 w-4 shrink-0 accent-[#0f8b8d]"
                />
                <span>
                  Akceptuję{' '}
                  <a href="/regulamin.html" target="_blank" rel="noopener" className="font-bold text-sea underline">regulamin</a>
                  {' '}i{' '}
                  <a href="/privacy.html" target="_blank" rel="noopener" className="font-bold text-sea underline">politykę prywatności</a>{' '}
                  SeaSteps.
                </span>
              </label>
            </>
          )}

          {error && (
            <div className="mt-3 flex items-center gap-2 rounded-2xl bg-red-50 px-4 py-3 text-sm font-semibold text-red-600">
              <Warning size={16} weight="fill" /> {error}
            </div>
          )}

          <button
            onClick={submit}
            disabled={loading}
            className="mt-6 flex w-full items-center justify-center gap-2 rounded-2xl bg-gradient-to-br from-sea to-deep py-3.5 text-base font-bold text-white shadow-[0_16px_30px_rgba(12,90,113,0.25)] transition active:scale-95 disabled:opacity-60"
          >
            {loading ? 'Chwilka…' : tab === 'login' ? 'Zaloguj się' : 'Załóż konto'} {!loading && <ArrowRight size={18} />}
          </button>

          <button
            onClick={sendMagicLink}
            disabled={loading}
            className="mt-3 w-full text-center text-sm font-bold text-sea disabled:opacity-60"
          >
            albo wyślij magiczny link →
          </button>
          {magicMsg && <p className="mt-2 text-center text-sm font-semibold text-[#2f7a45]">{magicMsg}</p>}
          <p className="mt-3 text-center text-[11px] leading-snug text-muted">
            Logując się, akceptujesz{' '}
            <a href="/regulamin.html" target="_blank" rel="noopener" className="underline">regulamin</a>
            {' '}i{' '}
            <a href="/privacy.html" target="_blank" rel="noopener" className="underline">politykę prywatności</a>.
          </p>
        </div>
      </div>
    </div>
  )
}
