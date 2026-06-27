import type { ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { CaretLeft } from '@phosphor-icons/react'

export function Card({
  children,
  className = '',
  onClick,
}: {
  children: ReactNode
  className?: string
  onClick?: () => void
}) {
  return (
    <div
      onClick={onClick}
      className={`glass rounded-[var(--radius-card)] shadow-[0_18px_40px_rgba(12,90,113,0.10)] ${
        onClick ? 'cursor-pointer active:scale-[0.99] transition-transform' : ''
      } ${className}`}
    >
      {children}
    </div>
  )
}

export function Pill({ children, tone = 'sea' }: { children: ReactNode; tone?: 'sea' | 'leaf' | 'sand' | 'muted' }) {
  const tones = {
    sea: 'bg-sea/10 text-deep',
    leaf: 'bg-leaf/15 text-[#2f7a45]',
    sand: 'bg-sand/25 text-[#8a6418]',
    muted: 'bg-white/70 text-muted',
  }
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-bold ${tones[tone]}`}>
      {children}
    </span>
  )
}

export function ProgressBar({ value, label }: { value: number; label?: string }) {
  return (
    <div>
      {label && (
        <div className="mb-1.5 flex justify-between text-xs font-bold text-muted">
          <span>{label}</span>
          <span className="text-deep">{value}%</span>
        </div>
      )}
      <div className="h-2.5 overflow-hidden rounded-full bg-sea/10">
        <div
          className="h-full rounded-full bg-gradient-to-r from-sea to-leaf transition-[width] duration-700"
          style={{ width: `${value}%` }}
        />
      </div>
    </div>
  )
}

export function ScreenHeader({
  title,
  subtitle,
  emoji,
  showBack = true,
}: {
  title: string
  subtitle?: string
  emoji?: string
  showBack?: boolean
}) {
  const navigate = useNavigate()
  const goBack = () => (window.history.length > 2 ? navigate(-1) : navigate('/'))
  return (
    <header className="px-5 pb-2 pt-4">
      {showBack && (
        <button
          onClick={goBack}
          aria-label="Wróć"
          className="mb-2 inline-flex items-center gap-1 rounded-full glass py-1.5 pl-2 pr-3 text-sm font-bold text-deep transition active:scale-95"
        >
          <CaretLeft size={18} /> Wróć
        </button>
      )}
      <h1 className="font-display text-[28px] font-bold leading-none text-ink">
        {emoji && <span className="mr-2">{emoji}</span>}
        {title}
      </h1>
      {subtitle && <p className="mt-1.5 text-sm leading-snug text-muted">{subtitle}</p>}
    </header>
  )
}

export function PrimaryButton({
  children,
  onClick,
  className = '',
  type = 'button',
}: {
  children: ReactNode
  onClick?: () => void
  className?: string
  type?: 'button' | 'submit'
}) {
  return (
    <button
      type={type}
      onClick={onClick}
      className={`inline-flex items-center justify-center gap-2 rounded-2xl bg-gradient-to-br from-sea to-deep px-5 py-3.5 text-[15px] font-bold text-white shadow-[0_16px_30px_rgba(12,90,113,0.25)] transition active:scale-[0.97] ${className}`}
    >
      {children}
    </button>
  )
}

export function SoftButton({
  children,
  onClick,
  className = '',
}: {
  children: ReactNode
  onClick?: () => void
  className?: string
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`inline-flex items-center justify-center gap-2 rounded-2xl border border-white/70 bg-white/80 px-5 py-3.5 text-[15px] font-bold text-deep transition active:scale-[0.97] ${className}`}
    >
      {children}
    </button>
  )
}
