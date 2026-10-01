import { Capacitor } from '@capacitor/core'
import { SITE_ORIGIN } from './site'
import { API_BASE, apiRequest, getToken, hasBackend, setToken } from './http'

const KEY = 'ss-auth'
const UID_KEY = 'ss-uid'

export function isAuthed(): boolean {
  try {
    return localStorage.getItem(KEY) === '1'
  } catch {
    return false
  }
}

export function setAuthed(v: boolean) {
  if (v) {
    localStorage.setItem(KEY, '1')
  } else {
    localStorage.removeItem(KEY)
    localStorage.removeItem(UID_KEY)
    setToken(null)
  }
}

/** Id zalogowanego użytkownika (z backendu), do oznaczania "to ja" na listach. */
export function currentUserId(): string | null {
  try {
    return localStorage.getItem(UID_KEY)
  } catch {
    return null
  }
}

export function setCurrentUserId(id: string | null): void {
  try {
    if (id) localStorage.setItem(UID_KEY, id)
    else localStorage.removeItem(UID_KEY)
  } catch {
    /* ignore */
  }
}

interface AuthProfile {
  id?: string
}

/** Logowanie e-mail + hasło. POST /auth/login, zapis tokenu JWT i aktualnego user ID. Rzuca ApiError przy błędzie. */
export async function login(email: string, password: string): Promise<void> {
  const res = await apiRequest<AuthProfile>('/auth/login', {
    method: 'POST',
    auth: false,
    body: { email, password },
  })
  if (res.token) setToken(res.token)

  // Starszy backend loginu zwracał tylko token, bez profilu. Wtedy po loginie
  // hasłem `ss-uid` zostawał pusty/stary, a historia spaceru odfiltrowywała
  // własne punkty GPS i mapa wyglądała jak „bez trasy”. Po zapisaniu tokenu
  // dociągamy /me tym samym klientem i ustawiamy ID z autorytatywnego profilu.
  const profile = res.data?.id ? res : await apiRequest<AuthProfile>('/me')
  if (profile.data?.id) setCurrentUserId(profile.data.id)

  setAuthed(true)
}

/** Rejestracja konta (POST /auth/register). Wymaga zgody na regulamin+politykę (RODO). */
export async function register(
  email: string,
  password: string,
  displayName: string,
  acceptedTerms: boolean,
): Promise<void> {
  const res = await apiRequest<{ id?: string }>('/auth/register', {
    method: 'POST',
    auth: false,
    body: { email, password, display_name: displayName, accepted_terms: acceptedTerms },
  })
  if (res.token) setToken(res.token)
  if (res.data?.id) setCurrentUserId(res.data.id)
  setAuthed(true)
}

// ── Weryfikacja e-maila (spec 2026-07-13) ───────────────────────────────────

/** Wyślij (ponownie) mail z linkiem weryfikacyjnym dla zalogowanego usera. */
export async function requestEmailVerification(): Promise<void> {
  await apiRequest('/auth/verify-email/request', { method: 'POST' })
}

/** Potwierdź e-mail tokenem z maila (nie loguje — tylko potwierdza skrzynkę). */
export async function confirmEmailVerification(token: string): Promise<void> {
  await apiRequest('/auth/verify-email/confirm', {
    method: 'POST',
    auth: false,
    body: { token },
  })
}

// ── RODO (spec 2026-07-13) ───────────────────────────────────────────────────

/** Usuń konto (DELETE /me) i wyczyść stan lokalny. Nieodwracalne. */
export async function deleteAccount(): Promise<void> {
  await apiRequest('/me', { method: 'DELETE' })
  setAuthed(false)
}

/** Pobierz pełny eksport danych (RODO art. 20) jako plik JSON. */
export async function downloadMyData(): Promise<void> {
  const res = await fetch(`${API_BASE}/api/v1/me/export`, {
    headers: { Authorization: `Bearer ${getToken() ?? ''}` },
  })
  if (!res.ok) throw new Error('Eksport danych nie powiódł się. Spróbuj za chwilę.')
  const filename = `seasteps-export-${new Date().toISOString().slice(0, 10)}.json`

  // Natywna apka: WebView ignoruje <a download> — zapis do cache apki
  // i systemowy arkusz udostępniania (zapis do Plików, mail, Dysk…).
  if (Capacitor.isNativePlatform()) {
    const text = await res.text()
    const [{ Filesystem, Directory, Encoding }, { Share }] = await Promise.all([
      import('@capacitor/filesystem'),
      import('@capacitor/share'),
    ])
    const { uri } = await Filesystem.writeFile({
      path: filename,
      data: text,
      directory: Directory.Cache,
      encoding: Encoding.UTF8,
    })
    try {
      await Share.share({ title: 'Eksport danych SeaSteps', files: [uri], dialogTitle: 'Zapisz lub wyślij eksport' })
    } catch (e) {
      // Zamknięcie arkusza bez wyboru to nie błąd eksportu.
      const msg = e instanceof Error ? e.message.toLowerCase() : ''
      if (!msg.includes('cancel')) throw e
    }
    return
  }

  const blob = await res.blob()
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

/** Wylogowanie. Best-effort POST /auth/logout, potem czyści stan lokalny. */
export async function logout(): Promise<void> {
  if (hasBackend()) {
    try {
      await apiRequest('/auth/logout', { method: 'POST' })
    } catch {
      /* i tak czyścimy lokalnie */
    }
  }
  setAuthed(false)
}

// ── Magic link via Supabase Auth ───────────────────────────────────────────
// Supabase sends the email + establishes a session on click; we then exchange
// its access token for THIS app's JWT so all data calls keep using the backend.

/** Send a Supabase magic-link email. Link returns to /auth/magic. */
export async function requestMagicLink(email: string): Promise<void> {
  const { supabase } = await import('./supabase')
  // natywna apka: App Link na stronę (https://seasteps.pl/app/auth/magic) przechwytywany przez apkę
  const redirectTo = Capacitor.isNativePlatform()
    ? `${SITE_ORIGIN}/app/auth/magic`
    : `${window.location.origin}${import.meta.env.BASE_URL}auth/magic`
  const { error } = await supabase.auth.signInWithOtp({
    email: email.trim(),
    options: { emailRedirectTo: redirectTo },
  })
  if (error) throw error
}

/**
 * Logowanie 8-cyfrowym kodem z maila (spec 2026-10-01) — niezawodna droga obok linku.
 * verifyOtp zapisuje sesję Supabase, potem ta sama wymiana co po kliknięciu linku.
 */
export async function verifyMagicCode(email: string, code: string): Promise<void> {
  const { supabase } = await import('./supabase')
  const token = code.replace(/\s+/g, '')
  const { error } = await supabase.auth.verifyOtp({ email: email.trim(), token, type: 'email' })
  if (error) {
    const msg = (error.message || '').toLowerCase()
    throw new Error(
      msg.includes('expired') || msg.includes('invalid')
        ? 'Kod jest nieprawidłowy albo wygasł. Sprawdź cyfry lub wyślij nowy link.'
        : 'Nie udało się zweryfikować kodu. Spróbuj ponownie.',
    )
  }
  if (!(await exchangeSupabaseSession())) throw new Error('Nie udało się zalogować kodem. Spróbuj ponownie.')
}

/**
 * After a Supabase magic-link redirect, exchange the Supabase session for the
 * backend JWT. Returns true on success. Clears the Supabase session afterwards.
 */
export async function exchangeSupabaseSession(): Promise<boolean> {
  const { supabase, hasSupabase } = await import('./supabase')
  if (!hasSupabase()) return false
  const { data } = await supabase.auth.getSession()
  const accessToken = data.session?.access_token
  if (!accessToken) return false

  // accepted_terms: klauzula zgody stoi pod formularzem magic-linka — backend
  // zapisuje ją tylko, gdy wymiana TWORZY nowe konto (RODO, spec 2026-07-13).
  const res = await apiRequest<{ id?: string }>('/auth/supabase', {
    method: 'POST',
    auth: false,
    body: { access_token: accessToken, accepted_terms: true },
  })
  if (res.token) setToken(res.token)
  if (res.data?.id) setCurrentUserId(res.data.id)
  setAuthed(true)
  await supabase.auth.signOut().catch(() => {})
  return true
}
