import { User, Buildings } from '@phosphor-icons/react'
import { useMode } from '../lib/mode'

export function ModeToggle() {
  const { mode, setMode } = useMode()
  return (
    <div className="inline-flex rounded-full bg-white/70 p-1 text-xs font-bold shadow-sm ring-1 ring-white/60">
      <button
        onClick={() => setMode('solo')}
        className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 transition ${
          mode === 'solo' ? 'bg-gradient-to-br from-sea to-deep text-white shadow' : 'text-muted'
        }`}
      >
        <User size={13} /> Ja
      </button>
      <button
        onClick={() => setMode('team')}
        className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 transition ${
          mode === 'team' ? 'bg-gradient-to-br from-sea to-deep text-white shadow' : 'text-muted'
        }`}
      >
        <Buildings size={13} /> Firma
      </button>
    </div>
  )
}
