import { useState } from 'react'
import { Bug } from '@phosphor-icons/react'
import { Card } from './ui'
import { ApiError } from '../lib/http'
import { BUG_CATEGORIES, DESCRIPTION_MAX, getLastWalkTrace, sendBugReport, type BugCategory } from '../lib/bugReport'

/**
 * Modal „Zgłoś błąd” (spec 2026-10-07): kategoria + opis + opcjonalnie zapis
 * ostatniego spaceru → POST /bug-reports. Wersja apki i model telefonu
 * dołączają się same.
 */
export function BugReportModal({
  screen,
  initialCategory = 'other',
  onClose,
}: {
  screen: string
  initialCategory?: BugCategory
  onClose: () => void
}) {
  const hasTrace = getLastWalkTrace() != null
  const [category, setCategory] = useState<BugCategory>(initialCategory)
  const [description, setDescription] = useState('')
  const [includeTrace, setIncludeTrace] = useState(hasTrace)
  const [sending, setSending] = useState(false)
  const [done, setDone] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = async () => {
    if (!description.trim() || sending) return
    setSending(true)
    setError(null)
    try {
      await sendBugReport({ category, description, screen, includeTrace: hasTrace && includeTrace })
      setDone(true)
    } catch (e) {
      if (e instanceof ApiError && e.status === 429) setError('Wysłano już kilka zgłoszeń w tej godzinie. Spróbuj później.')
      else if (e instanceof ApiError && e.status === 401) setError('Zaloguj się, żeby wysłać zgłoszenie.')
      else setError('Nie udało się wysłać zgłoszenia. Sprawdź internet i spróbuj ponownie.')
    } finally {
      setSending(false)
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/40 px-6"
      role="dialog"
      aria-modal="true"
      aria-labelledby="bug-title"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <Card className="max-h-[85vh] w-full max-w-sm overflow-y-auto p-5">
        <div className="flex items-center gap-2 text-deep">
          <Bug size={20} weight="fill" />
          <span id="bug-title" className="font-display text-lg font-bold">Zgłoś błąd</span>
        </div>

        {done ? (
          <>
            <p className="mt-3 text-sm text-ink">Dziękujemy. Zgłoszenie dotarło do zespołu SeaSteps i sprawdzimy je.</p>
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
            <p className="mt-2 text-sm text-muted">Czego dotyczy problem?</p>
            <div className="mt-3 space-y-1.5">
              {BUG_CATEGORIES.map((c) => (
                <label
                  key={c.value}
                  className={`flex cursor-pointer items-center gap-2 rounded-xl border px-3 py-2 text-sm font-semibold transition ${
                    category === c.value ? 'border-sea/40 bg-sea/10 text-ink' : 'border-white/70 bg-white/60 text-ink'
                  }`}
                >
                  <input
                    type="radio"
                    name="bug-category"
                    value={c.value}
                    checked={category === c.value}
                    onChange={() => setCategory(c.value)}
                    className="accent-teal-700"
                  />
                  {c.label}
                </label>
              ))}
            </div>
            <label htmlFor="bug-description" className="mt-3 block text-sm font-bold text-ink">Co się stało?</label>
            <textarea
              id="bug-description"
              value={description}
              onChange={(e) => setDescription(e.target.value.slice(0, DESCRIPTION_MAX))}
              placeholder="np. pauza włączała się, choć szłam; po 2 minutach kroki skoczyły o 200"
              rows={4}
              className="mt-1 w-full resize-none rounded-xl border border-white/70 bg-white/80 px-3 py-2 text-sm text-ink outline-none placeholder:text-muted/70 focus:ring-2 focus:ring-sea/30"
            />
            <div className="text-right text-[11px] font-semibold text-muted">{description.length}/{DESCRIPTION_MAX}</div>
            {hasTrace && (
              <label className="mt-1 flex cursor-pointer items-start gap-2 text-sm text-ink">
                <input type="checkbox" checked={includeTrace} onChange={(e) => setIncludeTrace(e.target.checked)} className="mt-0.5 accent-teal-700" />
                <span>
                  Dołącz zapis ostatniego spaceru
                  <span className="block text-xs text-muted">Kroki, czasy i trasa jako przesunięcia w metrach, bez adresu i współrzędnych. Pozwala odtworzyć problem krok po kroku.</span>
                </span>
              </label>
            )}
            <p className="mt-2 text-xs text-muted">Do zgłoszenia dołączamy wersję aplikacji i model telefonu.</p>
            {error && <p className="mt-2 text-xs font-semibold text-rose-600">{error}</p>}
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
                disabled={!description.trim() || sending}
                className="flex-1 rounded-2xl bg-gradient-to-br from-sea to-deep py-2.5 text-sm font-bold text-white transition active:scale-[0.98] disabled:opacity-40"
              >
                {sending ? 'Wysyłanie…' : 'Wyślij'}
              </button>
            </div>
          </>
        )}
      </Card>
    </div>
  )
}
