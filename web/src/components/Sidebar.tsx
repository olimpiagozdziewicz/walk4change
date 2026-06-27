import { Link, useLocation } from 'react-router-dom'
import { House, Footprints, Broadcast, UsersThree, CalendarHeart, Storefront, UserCircle } from '@phosphor-icons/react'
import { Logo } from './Logo'

const items = [
  { to: '/', label: 'Start', icon: House, owns: ['/', '/eco'] },
  { to: '/walk', label: 'Spacer', icon: Footprints, owns: ['/walk', '/history'] },
  { to: '/live', label: 'Na żywo', icon: Broadcast, owns: ['/live'] },
  { to: '/community', label: 'Ludzie', icon: UsersThree, owns: ['/community'] },
  { to: '/events', label: 'Eventy', icon: CalendarHeart, owns: ['/events'] },
  { to: '/partners', label: 'Partnerzy', icon: Storefront, owns: ['/partners'] },
  { to: '/profile', label: 'Profil', icon: UserCircle, owns: ['/profile'] },
]

/** Boczne menu — widok www (desktop). Na telefonie ukryte (jest bottom-nav). */
export function Sidebar() {
  const { pathname } = useLocation()
  return (
    <aside className="sticky top-0 hidden h-[100svh] w-60 shrink-0 flex-col items-start gap-1 border-r border-white/50 px-4 py-6 lg:flex">
      <div className="px-3 pb-6">
        <Logo />
      </div>
      {items.map(({ to, label, icon: Icon, owns }) => {
        const isActive = owns.includes(pathname)
        return (
          <Link
            key={to}
            to={to}
            className={`flex w-44 items-center gap-3 rounded-2xl px-4 py-2.5 text-sm font-bold transition ${
              isActive
                ? 'bg-gradient-to-br from-sea to-leaf text-white shadow-[0_10px_22px_rgba(15,139,141,0.3)]'
                : 'text-muted hover:bg-sea/10 hover:text-deep'
            }`}
          >
            <Icon size={20} weight="fill" /> {label}
          </Link>
        )
      })}
      <div className="mt-auto px-3 text-xs text-muted">SeaSteps · Hack4Change 2026</div>
    </aside>
  )
}
