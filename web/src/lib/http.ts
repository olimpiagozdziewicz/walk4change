/**
 * SeaSteps — klient HTTP do backendu (Rust/Axum).
 *
 * Bazę bierze z VITE_API_BASE. Puste => backend wyłączony i warstwa danych
 * (`lib/api.ts`) działa na mockach. Ustawione (np. http://localhost:8080)
 * => realne wywołania REST z tokenem JWT.
 *
 * Backend używa kopert: sukces `{ "data": ... , "meta"? }`, błąd
 * `{ "error": { "code", "message" } }`. Liczby (punkty, koszty) przychodzą
 * jako stringi (rust_decimal) — patrz `parseFloat` w adapterach api.ts.
 */

/** Bazowy adres backendu, bez końcowego ukośnika. '' => tryb mock. */
export const API_BASE: string = (import.meta.env.VITE_API_BASE ?? '').replace(/\/+$/, '')

const API_PREFIX = '/api/v1'
const TOKEN_KEY = 'ss-token'

/** Czy frontend ma podpięty realny backend. */
export function hasBackend(): boolean {
  return API_BASE.length > 0
}

export function getToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY)
  } catch {
    return null
  }
}

export function setToken(token: string | null): void {
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token)
    else localStorage.removeItem(TOKEN_KEY)
  } catch {
    /* storage niedostępny — ignorujemy */
  }
}

/** Błąd zwrócony przez backend (z kodem i statusem HTTP). */
export class ApiError extends Error {
  readonly code: string
  readonly status: number

  constructor(message: string, code: string, status: number) {
    super(message)
    this.name = 'ApiError'
    this.code = code
    this.status = status
  }
}

interface Envelope<T> {
  data?: T
  meta?: unknown
  token?: string
  error?: { code?: string; message?: string }
}

interface RequestOptions {
  method?: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE'
  body?: unknown
  /** Dołącz nagłówek Authorization: Bearer <token> (domyślnie true). */
  auth?: boolean
}

/**
 * Wywołanie API. Zwraca całą kopertę (data/token/meta). Rzuca ApiError przy
 * błędzie sieci lub odpowiedzi 4xx/5xx / `{ error }`.
 */
export async function apiRequest<T>(path: string, opts: RequestOptions = {}): Promise<Envelope<T>> {
  const { method = 'GET', body, auth = true } = opts

  const headers: Record<string, string> = { Accept: 'application/json' }
  if (body !== undefined) headers['Content-Type'] = 'application/json'
  if (auth) {
    const token = getToken()
    if (token) headers.Authorization = `Bearer ${token}`
  }

  let res: Response
  try {
    res = await fetch(`${API_BASE}${API_PREFIX}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    })
  } catch {
    throw new ApiError('Nie udało się połączyć z serwerem.', 'NETWORK', 0)
  }

  if (res.status === 204) return {}

  let json: Envelope<T> = {}
  try {
    json = (await res.json()) as Envelope<T>
  } catch {
    /* puste/niepoprawne ciało — zostaje {} */
  }

  if (!res.ok || json.error) {
    const code = json.error?.code ?? `HTTP_${res.status}`
    const message = json.error?.message ?? `Błąd serwera (${res.status}).`
    throw new ApiError(message, code, res.status)
  }

  return json
}
