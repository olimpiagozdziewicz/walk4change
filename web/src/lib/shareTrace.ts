/**
 * Udostępnienie zapisu spaceru (spec 2026-10-07-zapis-spaceru-do-zgloszenia-bledu).
 * Apka nic nie wysyła sama: plik idzie przez systemowy arkusz udostępniania,
 * a użytkownik wybiera, dokąd (mail, WhatsApp, Dysk).
 */
import type { TraceRecorder } from './walkTrace'

/** Model telefonu z user-agenta WebView, np. „RMX5051” — bez reszty UA. */
export function deviceModel(): string {
  const m = /Android [\d.]+; ([^;)]+)/.exec(navigator.userAgent)
  return m ? m[1].trim() : 'nieznany'
}

export async function shareWalkTrace(rec: TraceRecorder, note: string): Promise<void> {
  const [{ Filesystem, Directory, Encoding }, { Share }, { App }] = await Promise.all([
    import('@capacitor/filesystem'),
    import('@capacitor/share'),
    import('@capacitor/app'),
  ])
  const info = await App.getInfo().catch(() => null)
  const trace = { ...rec.snapshot(), app: info ? `${info.version} (${info.build})` : 'nieznana', note: note.slice(0, 500) }
  const filename = `seasteps-zapis-spaceru-${trace.day}.json`
  const { uri } = await Filesystem.writeFile({
    path: filename,
    data: JSON.stringify(trace),
    directory: Directory.Cache,
    encoding: Encoding.UTF8,
  })
  try {
    await Share.share({ title: 'Zapis spaceru SeaSteps', files: [uri], dialogTitle: 'Wyślij zapis spaceru' })
  } catch (e) {
    // Zamknięcie arkusza bez wyboru to nie błąd.
    const msg = e instanceof Error ? e.message.toLowerCase() : ''
    if (!msg.includes('cancel')) throw e
  }
}
