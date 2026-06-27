import { useEffect, useState } from 'react'
import { DownloadSimple, Share } from '@phosphor-icons/react'

interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>
}

function isIos(): boolean {
  return /iphone|ipad|ipod/i.test(navigator.userAgent)
}
function isStandalone(): boolean {
  return (
    window.matchMedia('(display-mode: standalone)').matches ||
    (navigator as unknown as { standalone?: boolean }).standalone === true
  )
}

/** Install-PWA affordance. Native prompt on Android/Chrome; Add-to-Home hint on iOS. */
export function InstallButton() {
  const [deferred, setDeferred] = useState<BeforeInstallPromptEvent | null>(null)
  const [installed, setInstalled] = useState(false)
  const [iosHint, setIosHint] = useState(false)

  useEffect(() => {
    const onPrompt = (e: Event) => {
      e.preventDefault()
      setDeferred(e as BeforeInstallPromptEvent)
    }
    const onInstalled = () => setInstalled(true)
    window.addEventListener('beforeinstallprompt', onPrompt)
    window.addEventListener('appinstalled', onInstalled)
    return () => {
      window.removeEventListener('beforeinstallprompt', onPrompt)
      window.removeEventListener('appinstalled', onInstalled)
    }
  }, [])

  if (installed || isStandalone()) return null

  const cls =
    'mt-4 flex w-full items-center justify-center gap-2 rounded-2xl border border-sea/30 bg-white/70 py-3 text-sm font-bold text-deep transition active:scale-[0.98]'

  // Android / desktop Chrome-Edge: native install prompt.
  if (deferred) {
    return (
      <button
        onClick={async () => {
          await deferred.prompt()
          await deferred.userChoice
          setDeferred(null)
        }}
        className={cls}
      >
        <DownloadSimple size={18} weight="fill" /> Zainstaluj aplikację SeaSteps
      </button>
    )
  }

  // iOS Safari: no beforeinstallprompt — show the Add-to-Home-Screen instructions.
  if (isIos()) {
    return (
      <div>
        <button onClick={() => setIosHint((v) => !v)} className={cls}>
          <Share size={18} weight="fill" /> Zainstaluj aplikację SeaSteps
        </button>
        {iosHint && (
          <p className="mt-2 text-center text-xs font-semibold text-muted">
            Na iPhonie: dotknij <b>Udostępnij</b> <span aria-hidden>⎙</span>, potem <b>„Dodaj do ekranu początkowego"</b>.
          </p>
        )}
      </div>
    )
  }

  return null
}
