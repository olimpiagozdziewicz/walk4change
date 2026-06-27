import { useEffect, useState } from 'react'
import { DownloadSimple } from '@phosphor-icons/react'

/** Pokazuje przycisk instalacji PWA, gdy przeglądarka go udostępnia. */
export function InstallButton() {
  const [deferred, setDeferred] = useState<any>(null)
  const [installed, setInstalled] = useState(false)

  useEffect(() => {
    const onPrompt = (e: any) => {
      e.preventDefault()
      setDeferred(e)
    }
    const onInstalled = () => setInstalled(true)
    window.addEventListener('beforeinstallprompt', onPrompt)
    window.addEventListener('appinstalled', onInstalled)
    return () => {
      window.removeEventListener('beforeinstallprompt', onPrompt)
      window.removeEventListener('appinstalled', onInstalled)
    }
  }, [])

  if (installed || !deferred) return null

  return (
    <button
      onClick={async () => {
        deferred.prompt()
        await deferred.userChoice
        setDeferred(null)
      }}
      className="mt-4 flex w-full items-center justify-center gap-2 rounded-2xl border border-sea/30 bg-white/70 py-3 text-sm font-bold text-deep transition active:scale-[0.98]"
    >
      <DownloadSimple size={18} weight="fill" /> Zainstaluj aplikację SeaSteps
    </button>
  )
}
