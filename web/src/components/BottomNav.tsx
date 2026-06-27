import { NavLink } from 'react-router-dom'
import { House, Footprints, UsersThree, CalendarHeart, UserCircle } from '@phosphor-icons/react'

const items = [
  { to: '/', label: 'Start', icon: House, end: true },
  { to: '/walk', label: 'Spacer', icon: Footprints, end: false },
  { to: '/community', label: 'Ludzie', icon: UsersThree, end: false },
  { to: '/events', label: 'Eventy', icon: CalendarHeart, end: false },
  { to: '/profile', label: 'Profil', icon: UserCircle, end: false },
]

export function BottomNav() {
  return (
    <nav className="glass absolute inset-x-0 bottom-0 z-20 flex items-stretch justify-around rounded-t-[28px] border-t border-white/70 px-2 pb-[max(10px,env(safe-area-inset-bottom))] pt-2 shadow-[0_-10px_30px_rgba(12,90,113,0.08)]">
      {items.map(({ to, label, icon: Icon, end }) => (
        <NavLink
          key={to}
          to={to}
          end={end}
          className="group flex flex-1 flex-col items-center gap-1 rounded-2xl py-1.5 text-muted transition"
        >
          {({ isActive }) => (
            <>
              <span
                className={`flex h-9 w-12 items-center justify-center rounded-full transition ${
                  isActive ? 'bg-gradient-to-br from-sea to-leaf text-white shadow-[0_8px_18px_rgba(15,139,141,0.35)]' : 'text-muted group-active:bg-sea/10'
                }`}
              >
                <Icon size={23} weight={isActive ? 'fill' : 'duotone'} />
              </span>
              <span className={`text-[11px] font-bold ${isActive ? 'text-deep' : 'text-muted'}`}>{label}</span>
            </>
          )}
        </NavLink>
      ))}
    </nav>
  )
}
