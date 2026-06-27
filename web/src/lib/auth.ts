import { apiRequest, hasBackend, setToken } from './http'

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

/**
 * Logowanie e-mail + hasło. Bez backendu (tryb mock) po prostu wpuszcza do apki.
 * Z backendem: POST /auth/login, zapis tokenu JWT. Rzuca ApiError przy błędzie.
 */
export async function login(email: string, password: string): Promise<void> {
  if (!hasBackend()) {
    setAuthed(true)
    return
  }
  const res = await apiRequest<unknown>('/auth/login', {
    method: 'POST',
    auth: false,
    body: { email, password },
  })
  if (res.token) setToken(res.token)
  setAuthed(true)
}

/** Rejestracja konta (POST /auth/register). Mock bez backendu. */
export async function register(email: string, password: string, displayName: string): Promise<void> {
  if (!hasBackend()) {
    setAuthed(true)
    return
  }
  const res = await apiRequest<{ id?: string }>('/auth/register', {
    method: 'POST',
    auth: false,
    body: { email, password, display_name: displayName },
  })
  if (res.token) setToken(res.token)
  if (res.data?.id) setCurrentUserId(res.data.id)
  setAuthed(true)
}

/** Wejście demo (gość) — zawsze tryb mock, bez realnego backendu. */
export function guestEnter(): void {
  setAuthed(true)
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
  const redirectTo = `${window.location.origin}/auth/magic`
  const { error } = await supabase.auth.signInWithOtp({
    email: email.trim(),
    options: { emailRedirectTo: redirectTo },
  })
  if (error) throw error
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

  const res = await apiRequest<{ id?: string }>('/auth/supabase', {
    method: 'POST',
    auth: false,
    body: { access_token: accessToken },
  })
  if (res.token) setToken(res.token)
  if (res.data?.id) setCurrentUserId(res.data.id)
  setAuthed(true)
  await supabase.auth.signOut().catch(() => {})
  return true
}
