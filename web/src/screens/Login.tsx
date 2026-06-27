import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Envelope, Lock, ArrowRight, Footprints, Leaf, UsersThree } from '@phosphor-icons/react'
import { Logo } from '../components/Logo'
import { FootstepTrail } from '../components/Footsteps'
import { setAuthed } from '../lib/auth'

type Tab = 'login' | 'signup'

export function Login() {
  const nav = useNavigate()
  const [tab, setTab] = useState<Tab>('login')
  const [email, setEmail] = useState('')
  const [pass, setPass] = useState('')
  const [pass2, setPass2] = useState('')

  const enter = () => {
    setAuthed(true)
    nav('/')
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
              onClick={() => setTab('login')}
              className={`flex-1 rounded-xl py-2.5 transition ${tab === 'login' ? 'bg-white text-deep shadow' : 'text-muted'}`}
            >
              Zaloguj się
            </button>
            <button
              onClick={() => setTab('signup')}
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
            </>
          )}

          <button
            onClick={enter}
            className="mt-6 flex w-full items-center justify-center gap-2 rounded-2xl bg-gradient-to-br from-sea to-deep py-3.5 text-base font-bold text-white shadow-[0_16px_30px_rgba(12,90,113,0.25)] transition active:scale-95"
          >
            {tab === 'login' ? 'Zaloguj się' : 'Załóż konto'} <ArrowRight size={18} />
          </button>

          <button onClick={enter} className="mt-3 w-full rounded-2xl border border-white/70 bg-white/80 py-3.5 text-sm font-bold text-deep transition active:scale-95">
            Wejdź jako gość (demo)
          </button>
        </div>
      </div>
    </div>
  )
}
