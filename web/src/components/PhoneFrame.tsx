import type { ReactNode } from 'react'
import { Footprints, Leaf, UsersThree, ArrowRight } from '@phosphor-icons/react'
import { BottomNav } from './BottomNav'
import { Logo } from './Logo'
import { FootstepTrail } from './Footsteps'

/**
 * Jedna responsywna apka:
 *  - telefon (mobile): pełnoekranowa apka
 *  - desktop (www): apka w ramce telefonu + panel-wizytówka obok (morze, hasło, stópki)
 */
export function PhoneFrame({ children }: { children: ReactNode }) {
  return (
    <div className="relative flex min-h-[100svh] items-center justify-center gap-10 overflow-hidden lg:gap-16 lg:px-12">
      {/* desktop sea backdrop */}
      <div className="pointer-events-none absolute inset-0 hidden lg:block">
        <div className="absolute inset-x-0 bottom-0 h-[42%] bg-gradient-to-t from-sea/25 via-sea/8 to-transparent" />
        <div className="absolute -bottom-24 left-1/2 h-72 w-[140%] -translate-x-1/2 rounded-[50%] bg-sea/15 blur-3xl animate-drift" />
      </div>

      {/* desktop brand panel */}
      <aside className="relative z-10 hidden max-w-md flex-col lg:flex">
        <Logo />
        <h1 className="mt-8 font-display text-5xl font-bold leading-[1.05] tracking-tight text-ink">
          Spacer, który robi dobrze <span className="text-sea">Tobie</span> i{' '}
          <span className="text-leaf">naturze</span>.
        </h1>
        <p className="mt-5 max-w-sm text-lg leading-relaxed text-muted">
          Zamień zwykły spacer w coś więcej: ruch, kontakt z naturą, wspólne wyjścia, eventy
          społeczne i małe działania dla Bałtyku. Bez moralizowania, z lekkością.
        </p>

        <div className="mt-8 space-y-3">
          <Value icon={<Footprints size={18} />} text="Każdy krok to punkty — nad wodą liczą się podwójnie" />
          <Value icon={<Leaf size={18} />} text="Bonus za naturę ×3, za spacer z kimś ×1.5" />
          <Value icon={<UsersThree size={18} />} text="Eventy, społeczność i nagrody dla Bałtyku" />
        </div>

        <div className="mt-10 flex items-center gap-3 text-sm font-bold text-deep">
          <FootstepTrail count={4} color="#0f8b8d" />
          <span className="inline-flex items-center gap-1.5">
            Klikaj po aplikacji <ArrowRight size={16} />
          </span>
        </div>
      </aside>

      {/* phone */}
      <div className="relative z-10 flex h-[100svh] w-full max-w-[440px] flex-col overflow-hidden bg-gradient-to-b from-bg-2 to-bg shadow-[0_40px_120px_rgba(12,90,113,0.22)] sm:h-[900px] sm:max-h-[94svh] sm:rounded-[40px] sm:ring-1 sm:ring-white/60">
        {/* ambient sea glow inside the app */}
        <div className="pointer-events-none absolute -top-24 right-[-20%] h-72 w-72 rounded-full bg-sea/20 blur-3xl" />
        <div className="pointer-events-none absolute -top-10 left-[-15%] h-56 w-56 rounded-full bg-leaf/20 blur-3xl" />

        <main className="no-scrollbar relative z-10 flex-1 overflow-y-auto pb-28">{children}</main>

        <BottomNav />
      </div>
    </div>
  )
}

function Value({ icon, text }: { icon: ReactNode; text: string }) {
  return (
    <div className="flex items-center gap-3">
      <span className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-white/70 text-sea shadow-sm">
        {icon}
      </span>
      <span className="text-[15px] font-semibold text-ink/80">{text}</span>
    </div>
  )
}
