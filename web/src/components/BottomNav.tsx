import { Link, useLocation } from 'react-router-dom'
import { House, Footprints, Broadcast, UsersThree, CalendarHeart, UserCircle } from '@phosphor-icons/react'

const items = [
  { to: '/', label: 'Start', icon: House, owns: ['/', '/eco', '/partners'] },
  { to: '/walk', label: 'Spacer', icon: Footprints, owns: ['/walk', '/history'] },
  { to: '/live', label: 'Na żywo', icon: Broadcast, owns: ['/live'] },
  { to: '/community', label: 'Ludzie', icon: UsersThree, owns: ['/community'] },
  { to: '/events', label: 'Eventy', icon: CalendarHeart, owns: ['/events'] },
  { to: '/profile', label: 'Profil', icon: UserCircle, owns: ['/profile'] },
]

export function BottomNav() {
  const { pathname } = useLocation()
  return (
    <nav className="glass fixed inset-x-0 bottom-0 z-30 flex items-stretch justify-around border-t border-white/70 px-2 pb-[max(10px,env(safe-area-inset-bottom))] pt-2 shadow-[0_-10px_30px_rgba(12,90,113,0.08)] lg:hidden">
      {items.map(({ to, label, icon: Icon, owns }) => {
        const isActive = owns.includes(pathname)
        return (
          <Link
            key={to}
            to={to}
            className="group flex flex-1 flex-col items-center gap-1 rounded-2xl py-1.5 transition"
          >
            <span
              className={`flex h-9 w-12 items-center justify-center rounded-full transition ${
                isActive
                  ? 'bg-gradient-to-br from-sea to-leaf text-white shadow-[0_8px_18px_rgba(15,139,141,0.35)]'
                  : 'text-sea/55 group-active:bg-sea/10'
              }`}
            >
              <Icon size={22} weight="fill" />
            </span>
            <span className={`text-[11px] font-bold ${isActive ? 'text-deep' : 'text-muted'}`}>{label}</span>
          </Link>
        )
      })}
    </nav>
  )
}
