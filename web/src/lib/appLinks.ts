import { Capacitor } from '@capacitor/core'
import { App } from '@capacitor/app'

/**
 * Android App Links (spec 2026-10-01): https://seasteps.pl/app/auth/magic#access_token=…
 * Supabase (implicit flow) zwraca tokeny we fragmencie URL. Webview apki ma inny origin,
 * więc sesję ustawiamy ręcznie i kierujemy router na istniejący ekran /auth/magic.
 */
const MAGIC_PATH = '/app/auth/magic'

async function handleUrl(url: string | undefined, navigate: (to: string) => void) {
  if (!url) return
  let u: URL
  try { u = new URL(url) } catch { return }
  if (u.origin !== 'https://seasteps.pl' || !u.pathname.startsWith(MAGIC_PATH)) return
  const params = new URLSearchParams(u.hash.replace(/^#/, ''))
  const access_token = params.get('access_token')
  const refresh_token = params.get('refresh_token') ?? ''
  if (access_token) {
    try {
      const { supabase } = await import('./supabase')
      await supabase.auth.setSession({ access_token, refresh_token })
    } catch { /* ekran magic pokaże błąd i poprosi o nowy link */ }
  }
  navigate('/auth/magic')
}

/** Zwraca funkcję odpinającą nasłuch. No-op na webie. */
export function initAppLinks(navigate: (to: string) => void): () => void {
  if (!Capacitor.isNativePlatform()) return () => {}
  let handle: { remove: () => Promise<void> } | null = null
  let removed = false
  App.getLaunchUrl().then((r) => handleUrl(r?.url, navigate)).catch(() => {})
  App.addListener('appUrlOpen', (e) => { void handleUrl(e.url, navigate) })
    .then((h) => { if (removed) void h.remove(); else handle = h })
  return () => { removed = true; void handle?.remove() }
}
