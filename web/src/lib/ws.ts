/**
 * SeaSteps — klient WebSocket do żywego feedu spacerów (backend Rust/Axum).
 *
 * Protokół (zgodny z `crates/api/src/ws`):
 *   klient → serwer: auth (pierwsza ramka), subscribe, subscribe_leaderboard, ping
 *   serwer → klient: ping_scored, leaderboard_update, session_event, error
 *
 * Subskrypcja sesji wymaga bycia jej AKTYWNYM uczestnikiem (po stronie serwera).
 */
import { API_BASE, getToken } from './http'

export interface ScoredPing {
  session_id: string
  user_id: string
  seq: number
  point: { lat: number; lng: number }
  segment_meters: string
  nature_mult: string
  together_mult: string
  points: string
  participant_total: string
}

export interface LeaderRow {
  user_id: string
  display_name: string
  total_points: string
}

interface Handlers {
  onOpen?: () => void
  onClose?: () => void
  onError?: (message: string) => void
  onPingScored?: (ping: ScoredPing) => void
  onLeaderboard?: (rows: LeaderRow[]) => void
  onSessionEvent?: (data: unknown) => void
}

type ServerFrame =
  | { type: 'ping_scored'; data: ScoredPing }
  | { type: 'leaderboard_update'; data: LeaderRow[] }
  | { type: 'session_event'; data: unknown }
  | { type: 'error'; error?: { message?: string } }

/** Połączenie WS do feedu na żywo. Token JWT brany z localStorage przy connect(). */
export class LiveSocket {
  private ws: WebSocket | null = null
  private readonly handlers: Handlers

  constructor(handlers: Handlers) {
    this.handlers = handlers
  }

  connect(): void {
    const wsBase = API_BASE.replace(/^http/, 'ws') // http→ws, https→wss
    const ws = new WebSocket(`${wsBase}/api/v1/ws`)
    this.ws = ws

    ws.onopen = () => {
      ws.send(JSON.stringify({ type: 'auth', token: getToken() ?? '' }))
      this.handlers.onOpen?.()
    }

    ws.onmessage = (ev: MessageEvent<string>) => {
      let frame: ServerFrame
      try {
        frame = JSON.parse(ev.data) as ServerFrame
      } catch {
        return
      }
      switch (frame.type) {
        case 'ping_scored':
          this.handlers.onPingScored?.(frame.data)
          break
        case 'leaderboard_update':
          this.handlers.onLeaderboard?.(frame.data)
          break
        case 'session_event':
          this.handlers.onSessionEvent?.(frame.data)
          break
        case 'error':
          this.handlers.onError?.(frame.error?.message ?? 'Błąd serwera (WS)')
          break
      }
    }

    ws.onerror = () => this.handlers.onError?.('Błąd połączenia WebSocket')
    ws.onclose = () => this.handlers.onClose?.()
  }

  subscribeSession(sessionId: string): void {
    this.send({ type: 'subscribe', session_id: sessionId })
  }

  subscribeLeaderboard(): void {
    this.send({ type: 'subscribe_leaderboard' })
  }

  /** Wyślij ping GPS (dla realnego urządzenia / symulacji z przeglądarki). */
  sendPing(sessionId: string, seq: number, lat: number, lng: number): void {
    this.send({
      type: 'ping',
      session_id: sessionId,
      seq,
      lat,
      lng,
      recorded_at: new Date().toISOString(),
    })
  }

  close(): void {
    this.ws?.close()
    this.ws = null
  }

  private send(obj: unknown): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(obj))
    }
  }
}
