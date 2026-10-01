import { useState } from 'react'
import { Flag } from '@phosphor-icons/react'
import { Card } from './ui'
import { ApiError } from '../lib/http'
import { api, REPORT_REASONS, type ReportReason, type ReportTargetType } from '../lib/api'

export interface ReportTarget {
  type: ReportTargetType
  id: string
  /** Krótki opis celu do nagłówka, np. „wpis Ani” / „komentarz Bartka”. */
  label: string
}

/**
 * Modal „Zgłoś treść” (regulamin pkt 7, polityka UGC Google Play):
 * powód z listy + opcjonalny opis do 500 znaków → POST /reports.
 */
export function ReportModal({ target, onClose }: { target: ReportTarget; onClose: () => void }) {
  const [reason, setReason] = useState<ReportReason | null>(null)
  const [note, setNote] = useState('')
  const [sending, setSending] = useState(false)
  const [done, setDone] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = async () => {
    if (!reason || sending) return
    setSending(true)
    setError(null)
    try {
      await api.reportContent({ targetType: target.type, targetId: target.id, reason, note })
      setDone(true)
    } catch (e) {
      if (e instanceof ApiError && e.status === 429) setError('Za dużo zgłoszeń naraz — spróbuj za minutę.')
      else if (e instanceof ApiError && e.status === 404) setError('Ta treść już nie istnieje.')
      else setError('Nie udało się wysłać zgłoszenia — spróbuj ponownie.')
    } finally {
      setSending(false)
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/40 px-6"
      role="dialog"
      aria-modal="true"
      aria-labelledby="report-title"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <Card className="max-h-[85vh] w-full max-w-sm overflow-y-auto p-5">
        <div className="flex items-center gap-2 text-rose-600">
          <Flag size={20} weight="fill" />
          <span id="report-title" className="font-display text-lg font-bold">
            Zgłoś {target.label}
          </span>
        </div>

        {done ? (
          <>
            <p className="mt-3 text-sm text-ink">
              Dziękujemy. Zgłoszenie trafiło do moderacji SeaSteps — sprawdzimy je bez zbędnej zwłoki.
              Jeśli ktoś Cię nęka, możesz go też zablokować.
            </p>
            <button
              type="button"
              onClick={onClose}
              className="mt-4 w-full rounded-2xl bg-gradient-to-br from-sea to-deep py-2.5 text-sm font-bold text-white transition active:scale-[0.98]"
            >
              Zamknij
            </button>
          </>
        ) : (
          <>
            <p className="mt-2 text-sm text-muted">Dlaczego zgłaszasz tę treść?</p>
            <div className="mt-3 space-y-1.5">
              {REPORT_REASONS.map((r) => (
                <label
                  key={r.value}
                  className={`flex cursor-pointer items-center gap-2 rounded-xl border px-3 py-2 text-sm font-semibold transition ${
                    reason === r.value ? 'border-rose-300 bg-rose-50 text-ink' : 'border-white/70 bg-white/60 text-ink'
                  }`}
                >
                  <input
                    type="radio"
                    name="report-reason"
                    value={r.value}
                    checked={reason === r.value}
                    onChange={() => setReason(r.value)}
                    className="accent-rose-600"
                  />
                  {r.label}
                </label>
              ))}
            </div>
            <textarea
              value={note}
              onChange={(e) => setNote(e.target.value.slice(0, 500))}
              placeholder="Opcjonalnie: dodaj szczegóły (do 500 znaków)"
              rows={3}
              className="mt-3 w-full resize-none rounded-xl border border-white/70 bg-white/80 px-3 py-2 text-sm text-ink outline-none placeholder:text-muted/70 focus:ring-2 focus:ring-rose-200"
            />
            <div className="text-right text-[11px] font-semibold text-muted">{note.length}/500</div>
            {error && <p className="mt-1 text-xs font-semibold text-rose-600">{error}</p>}
            <div className="mt-3 flex gap-2">
              <button
                type="button"
                onClick={onClose}
                className="flex-1 rounded-2xl border border-white/70 bg-white/60 py-2.5 text-sm font-bold text-muted transition active:scale-[0.98]"
              >
                Anuluj
              </button>
              <button
                type="button"
                onClick={submit}
                disabled={!reason || sending}
                className="flex-1 rounded-2xl bg-rose-600 py-2.5 text-sm font-bold text-white transition active:scale-[0.98] disabled:opacity-40"
              >
                {sending ? 'Wysyłanie…' : 'Zgłoś'}
              </button>
            </div>
          </>
        )}
      </Card>
    </div>
  )
}
