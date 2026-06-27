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

    // pokaż okno po chwili (zdąży złapać beforeinstallprompt)
    const t = window.setTimeout(() => setOpen(true), 1200)
    return () => {
      window.removeEventListener('beforeinstallprompt', onPrompt)
      window.clearTimeout(t)
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
          className="fixed inset-0 z-[60] flex items-end justify-center bg-[rgba(12,90,113,0.28)] p-0 backdrop-blur-sm sm:items-center sm:p-6"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={dismiss}
        >
          <motion.div
            onClick={(e) => e.stopPropagation()}
            initial={{ y: 60, opacity: 0, scale: 0.98 }}
            animate={{ y: 0, opacity: 1, scale: 1 }}
            exit={{ y: 60, opacity: 0 }}
            transition={{ type: 'spring', stiffness: 320, damping: 30 }}
            className="relative w-full max-w-sm rounded-t-[28px] bg-white p-6 text-center shadow-[0_-20px_60px_rgba(12,90,113,0.25)] sm:rounded-[28px]"
          >
            <button onClick={dismiss} aria-label="Zamknij" className="absolute right-4 top-4 grid h-8 w-8 place-items-center rounded-full bg-sea/8 text-muted">
              <X size={16} weight="bold" />
            </button>

            <div className="mx-auto mb-3 drop-shadow-[0_12px_24px_rgba(15,139,141,0.3)]">
              <LogoMark size={68} />
            </div>
            <h2 className="font-display text-2xl font-bold text-ink">Zainstaluj SeaSteps</h2>
            <p className="mx-auto mt-2 max-w-[280px] text-sm leading-snug text-muted">
              Dodaj aplikację do ekranu głównego — pełny ekran, szybki dostęp i działa też offline. Bez sklepu, jednym dotknięciem.
            </p>

            {ios ? (
              <div className="mt-5 rounded-2xl bg-sea/8 p-4 text-left text-sm font-semibold text-deep">
                <span className="inline-flex items-center gap-2">
                  Na iPhone: dotknij <Export size={18} weight="fill" className="text-sea" /> <b>Udostępnij</b>
                </span>
                <div className="mt-1">→ przewiń i wybierz <b>„Do ekranu początkowego"</b>.</div>
              </div>
            ) : deferred ? (
              <button
                onClick={install}
                className="mt-5 flex w-full items-center justify-center gap-2 rounded-2xl bg-gradient-to-br from-sea to-deep py-3.5 text-base font-bold text-white shadow-[0_16px_30px_rgba(12,90,113,0.25)] transition active:scale-95"
              >
                <DownloadSimple size={20} weight="fill" /> Zainstaluj
              </button>
            ) : (
              <div className="mt-5 rounded-2xl bg-sea/8 p-4 text-sm font-semibold text-deep">
                W menu przeglądarki wybierz <b>„Zainstaluj aplikację"</b> / <b>„Dodaj do ekranu głównego"</b>.
              </div>
            )}

            <button onClick={dismiss} className="mt-3 text-sm font-bold text-muted">
              Może później
            </button>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
