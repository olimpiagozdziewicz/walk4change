import type { ReactNode } from 'react'
import { Sidebar } from './Sidebar'
import { BottomNav } from './BottomNav'
import { InstallModal } from './InstallModal'

/**
 * Responsywny shell aplikacji:
 *  - desktop (www): boczne menu + wyśrodkowana kolumna treści
 *  - telefon: pełny ekran + dolne menu (bottom-nav)
 * Ten sam kod „składa się" z www do telefonu wraz ze zwężaniem okna.
 */
export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="mx-auto flex min-h-[100svh] w-full max-w-6xl">
      <Sidebar />
      {/* min-w-0: bez tego flex item rośnie do min-content najszerszego dziecka
          (np. niełamliwy wiersz karty) i rozpycha stronę poza viewport na mobile */}
      <div className="relative flex min-h-[100svh] min-w-0 flex-1 flex-col overflow-x-clip">
        {/* ambient sea glow */}
        <div className="pointer-events-none absolute -top-24 right-0 h-72 w-72 rounded-full bg-sea/15 blur-3xl" />
        <main className="no-scrollbar relative z-10 flex-1 overflow-x-hidden pb-28 lg:pb-12">
          <div className="mx-auto w-full lg:max-w-2xl">{children}</div>
        </main>
        <BottomNav />
      </div>
      <InstallModal />
    </div>
  )
}
