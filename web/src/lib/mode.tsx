import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'

export type Mode = 'solo' | 'team'

const ModeCtx = createContext<{ mode: Mode; setMode: (m: Mode) => void }>({
  mode: 'solo',
  setMode: () => {},
})

export function ModeProvider({ children }: { children: ReactNode }) {
  const [mode, setMode] = useState<Mode>(() => {
    const saved = typeof localStorage !== 'undefined' ? localStorage.getItem('ss-mode') : null
    return saved === 'team' ? 'team' : 'solo'
  })
  useEffect(() => {
    localStorage.setItem('ss-mode', mode)
  }, [mode])
  return <ModeCtx.Provider value={{ mode, setMode }}>{children}</ModeCtx.Provider>
}

export const useMode = () => useContext(ModeCtx)
