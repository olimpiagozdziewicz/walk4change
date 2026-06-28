import { useEffect, useState } from 'react'
import { motion, AnimatePresence } from 'motion/react'
import { DownloadSimple, X, Export } from '@phosphor-icons/react'
import { LogoMark } from './Logo'

const DISMISS_KEY = 'ss-install-dismissed'

function isStandalone() {
  return (
    window.matchMedia('(display-mode: standalone)').matches ||
    // @ts-expect-error iOS Safari
    window.navigator.standalone === true
  )
}
function isIOS() {
  return /iphone|ipad|ipod/i.test(navigator.userAgent)
}

/** Małe, nienachalne okienko instalacji w rogu — pojawia się po interakcji + chwili. */
export function InstallModal() {
  const [open, setOpen] = useState(false)
  const [deferred, setDeferred] = useState<any>(null)
  const ios = isIOS()

  useEffect(() => {
    if (isStandalone()) return
    if (localStorage.getItem(DISMISS_KEY) === '1') return

    const onPrompt = (e: any) => {
      e.preventDefault()
      setDeferred(e)
    }
    window.addEventListener('beforeinstallprompt', onPrompt)
    window.addEventListener('appinstalled', () => setOpen(false))

    // pokaż dopiero ~6 s PO pierwszej interakcji (scroll/klik), nie od razu
    let timer: number | undefined
    let used = false
    const arm = () => {
      if (used) return
      used = true
      timer = window.setTimeout(() => setOpen(true), 6000)
    }
    window.addEventListener('scroll', arm, { once: true, passive: true })
    window.addEventListener('pointerdown', arm, { once: true })
    window.addEventListener('keydown', arm, { once: true })

    return () => {
      window.removeEventListener('beforeinstallprompt', onPrompt)
      window.removeEventListener('scroll', arm)
      window.removeEventListener('pointerdown', arm)
      window.removeEventListener('keydown', arm)
      if (timer) window.clearTimeout(timer)
    }
  }, [])

  const dismiss = () => {
    localStorage.setItem(DISMISS_KEY, '1')
    setOpen(false)
  }

  const install = async () => {
    if (!deferred) return
    deferred.prompt()
    await deferred.userChoice
    setDeferred(null)
    setOpen(false)
  }

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0, y: 24, scale: 0.96 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 24, scale: 0.96 }}
          transition={{ type: 'spring', stiffness: 320, damping: 28 }}
          className="fixed left-4 right-4 z-[60] rounded-3xl border border-black/5 bg-white p-4 shadow-[0_24px_60px_rgba(12,90,113,0.28)] sm:left-auto sm:right-6 sm:w-[330px] bottom-[calc(env(safe-area-inset-bottom,0px)+88px)] sm:bottom-6"
        >
          <button onClick={dismiss} aria-label="Zamknij" className="absolute right-3 top-3 grid h-7 w-7 place-items-center rounded-full bg-sea/8 text-muted">
            <X size={14} weight="bold" />
          </button>

          <div className="flex items-start gap-3 pr-6">
            <div className="shrink-0 drop-shadow-[0_8px_18px_rgba(15,139,141,0.3)]">
              <LogoMark size={44} />
            </div>
            <div>
              <div className="font-display text-base font-bold text-ink">Zainstaluj SeaSteps</div>
              <p className="mt-0.5 text-xs leading-snug text-muted">Pełny ekran, szybki dostęp, działa offline — bez sklepu.</p>
            </div>
          </div>

          {ios ? (
            <p className="mt-3 rounded-2xl bg-sea/8 px-3 py-2 text-xs font-semibold text-deep">
              Dotknij <Export size={14} weight="fill" className="inline align-text-bottom text-sea" /> <b>Udostępnij</b> → <b>„Do ekranu początkowego"</b>
            </p>
          ) : deferred ? (
            <button
              onClick={install}
              className="mt-3 flex w-full items-center justify-center gap-2 rounded-2xl bg-gradient-to-br from-sea to-deep py-2.5 text-sm font-bold text-white transition active:scale-95"
            >
              <DownloadSimple size={16} weight="fill" /> Zainstaluj
            </button>
          ) : (
            <p className="mt-3 rounded-2xl bg-sea/8 px-3 py-2 text-xs font-semibold text-deep">
              W menu przeglądarki: <b>„Zainstaluj aplikację"</b>
            </p>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  )
}
