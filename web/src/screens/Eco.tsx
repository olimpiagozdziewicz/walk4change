import { useEffect, useRef, useState, type ReactNode } from 'react'
import { motion } from 'motion/react'
import { Camera, MapPin, PaperPlaneTilt, CheckCircle, Warning, Sparkle, Leaf } from '@phosphor-icons/react'
import { ScreenHeader, Card, Pill, PrimaryButton } from '../components/ui'
import { Glyph } from '../components/Glyph'
import { Celebrate } from '../components/Celebrate'
import { api, type EcoReport, type Reward } from '../lib/api'

const statusMeta: Record<EcoReport['status'], { label: string; tone: 'leaf' | 'sand' | 'sea' }> = {
  cleaned: { label: '✓ posprzątane', tone: 'leaf' },
  reported: { label: '⏳ zgłoszone', tone: 'sand' },
  open: { label: 'otwarte', tone: 'sea' },
}

type Tab = 'report' | 'done'

export function Eco() {
  const [tab, setTab] = useState<Tab>('report')
  const [reports, setReports] = useState<EcoReport[]>([])
  const [rewards, setRewards] = useState<Reward[]>([])
  const [sent, setSent] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [desc, setDesc] = useState('')
  const [category, setCategory] = useState<string | null>(null)
  const [photoBefore, setPhotoBefore] = useState<File | null>(null)
  const [photoAfter, setPhotoAfter] = useState<File | null>(null)
  const [reportPhoto, setReportPhoto] = useState<File | null>(null)

  const loadReports = () => { api.getEcoReports().then(setReports).catch(() => {}) }

  useEffect(() => {
    loadReports()
    api.getRewards().then(setRewards)
  }, [])

  const switchTab = (t: Tab) => {
    setTab(t)
    setSent(false)
    setError(null)
    setDesc('')
    setCategory(null)
    setPhotoBefore(null)
    setPhotoAfter(null)
    setReportPhoto(null)
  }

  const submit = async () => {
    if (busy) return
    setBusy(true)
    setError(null)
    try {
      // Photos go straight to Supabase Storage; only their URLs hit the API.
      const [pUrl, pBefore, pAfter] = await Promise.all([
        reportPhoto ? api.uploadEcoPhoto(reportPhoto) : Promise.resolve(null),
        photoBefore ? api.uploadEcoPhoto(photoBefore) : Promise.resolve(null),
        photoAfter ? api.uploadEcoPhoto(photoAfter) : Promise.resolve(null),
      ])
      await api.createEcoReport({
        kind: tab === 'report' ? 'report' : 'cleanup',
        category: category ?? '',
        description: desc,
        photoUrl: pUrl,
        photoBeforeUrl: pBefore,
        photoAfterUrl: pAfter,
      })
      loadReports()
      setSent(true)
    } catch {
      setError('Nie udało się wysłać. Spróbuj ponownie.')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div>
      <ScreenHeader title="Eko" icon={<Leaf size={22} />} subtitle="Coś da się posprzątać — pochwal się. Większy problem — zgłoś." />

      <div className="space-y-4 px-5 pt-2">
        {/* segmented tabs */}
        <div className="inline-flex w-full rounded-2xl bg-white/70 p-1 text-sm font-bold ring-1 ring-white/60">
          <button
            onClick={() => switchTab('report')}
            className={`flex flex-1 items-center justify-center gap-1.5 rounded-xl py-2.5 transition ${
              tab === 'report' ? 'bg-gradient-to-br from-sea to-deep text-white shadow' : 'text-muted'
            }`}
          >
            <Warning size={15} /> Zgłoś problem
          </button>
          <button
            onClick={() => switchTab('done')}
            className={`flex flex-1 items-center justify-center gap-1.5 rounded-xl py-2.5 transition ${
              tab === 'done' ? 'bg-gradient-to-br from-leaf to-sea text-white shadow' : 'text-muted'
            }`}
          >
            <Sparkle size={15} /> Pochwal się
          </button>
        </div>

        {/* form card */}
        <Card className="p-5">
          {sent ? (
            <motion.div
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              className="relative flex flex-col items-center py-4 text-center"
            >
              <Celebrate pieces={28} />
              <CheckCircle size={48} className="text-leaf" />
              {tab === 'report' ? (
                <>
                  <p className="mt-2 font-bold text-deep">Dzięki! Zgłoszenie wysłane.</p>
                  <p className="text-sm text-muted">+15 pkt za czujność 🌱</p>
                </>
              ) : (
                <>
                  <p className="mt-2 font-bold text-deep">Brawo! 🌟</p>
                  <p className="text-sm text-muted">+25 pkt — i dajesz dobry przykład innym 🌊</p>
                </>
              )}
              <button onClick={() => switchTab(tab)} className="mt-3 text-sm font-bold text-sea">
                {tab === 'report' ? 'Zgłoś kolejne' : 'Pochwal się znowu'}
              </button>
            </motion.div>
          ) : tab === 'report' ? (
            <div className="space-y-3">
              <h2 className="font-display text-lg font-bold text-ink">Zgłoś problem</h2>
              <div className="flex flex-wrap gap-2">
                {['Śmieci', 'Rozlewisko', 'Dzikie wysypisko', 'Inne'].map((t) => (
                  <button key={t} type="button" onClick={() => setCategory(t)}>
                    <Pill tone={category === t ? 'sea' : 'muted'}>{t}</Pill>
                  </button>
                ))}
              </div>
              <textarea
                value={desc}
                onChange={(e) => setDesc(e.target.value)}
                placeholder="Opisz krótko, co widzisz…"
                rows={3}
                className="w-full resize-none rounded-2xl border border-white/70 bg-white/70 px-4 py-3 text-sm text-ink outline-none placeholder:text-muted/70 focus:ring-2 focus:ring-sea/30"
              />
              <div className="grid grid-cols-2 gap-2.5">
                <UploadBtn icon={<Camera size={18} />} label="Zdjęcie" onFile={setReportPhoto} />
                <UploadBtn icon={<MapPin size={18} />} label="Lokalizacja" />
              </div>
              {error && <p className="text-sm font-semibold text-rose-600">{error}</p>}
              <PrimaryButton onClick={submit} disabled={busy} className="w-full">
                <PaperPlaneTilt size={18} /> {busy ? 'Wysyłam…' : 'Wyślij zgłoszenie'}
              </PrimaryButton>
            </div>
          ) : (
            <div className="space-y-3">
              <h2 className="font-display text-lg font-bold text-ink">Pochwal się — posprzątane!</h2>
              <p className="-mt-1 text-sm text-muted">Coś ogarnęłaś sama? Pokaż efekt i zgarnij punkty.</p>
              <div className="flex flex-wrap gap-2">
                {['Plaża', 'Park', 'Las', 'Ulica', 'Brzeg'].map((t) => (
                  <button key={t} type="button" onClick={() => setCategory(t)}>
                    <Pill tone={category === t ? 'sea' : 'leaf'}>{t}</Pill>
                  </button>
                ))}
              </div>
              <textarea
                value={desc}
                onChange={(e) => setDesc(e.target.value)}
                placeholder="Co posprzątałaś? Np. worek śmieci z plaży w Brzeźnie…"
                rows={3}
                className="w-full resize-none rounded-2xl border border-white/70 bg-white/70 px-4 py-3 text-sm text-ink outline-none placeholder:text-muted/70 focus:ring-2 focus:ring-leaf/30"
              />
              <div className="grid grid-cols-2 gap-2.5">
                <UploadBtn icon={<Camera size={18} />} label="Zdjęcie PRZED" tone="leaf" onFile={setPhotoBefore} />
                <UploadBtn icon={<Camera size={18} />} label="Zdjęcie PO" tone="leaf" onFile={setPhotoAfter} />
              </div>
              {error && <p className="text-sm font-semibold text-rose-600">{error}</p>}
              <PrimaryButton onClick={submit} disabled={busy} className="w-full bg-gradient-to-br from-leaf to-sea">
                <Sparkle size={18} /> {busy ? 'Wysyłam…' : 'Pochwal się'}
              </PrimaryButton>
            </div>
          )}
        </Card>

        {/* recent activity */}
        <div>
          <h2 className="mb-2 font-display text-lg font-bold text-ink">Ostatnia aktywność</h2>
          <div className="space-y-2.5">
            {reports.length === 0 && (
              <p className="text-sm text-muted">Brak zgłoszeń jeszcze — bądź pierwsza!</p>
            )}
            {reports.map((r) => {
              const thumb = r.photoAfterUrl || r.photoUrl || r.photoBeforeUrl
              return (
                <Card key={r.id} className="flex items-center gap-3 p-3.5">
                  {thumb ? (
                    <img src={thumb} alt="" className="h-12 w-12 shrink-0 rounded-xl object-cover" />
                  ) : (
                    <div className="grid h-10 w-10 place-items-center rounded-xl bg-sea/10 text-sea">
                      {r.status === 'cleaned' ? <Sparkle size={18} /> : <Leaf size={18} />}
                    </div>
                  )}
                  <div className="flex-1 min-w-0">
                    <div className="text-sm font-bold text-ink">
                      {r.type}
                      {r.author && <span className="font-semibold text-muted"> • {r.author}</span>}
                    </div>
                    <div className="truncate text-xs text-muted">{r.description || r.location}</div>
                  </div>
                  <Pill tone={statusMeta[r.status].tone}>{statusMeta[r.status].label}</Pill>
                </Card>
              )
            })}
          </div>
        </div>

        {/* rewards */}
        <div>
          <h2 className="mb-2 font-display text-lg font-bold text-ink">Nagrody</h2>
          <div className="grid grid-cols-3 gap-2.5">
            {rewards.map((rw) => (
              <Card key={rw.id} className="p-3 text-center">
                <div className="mx-auto grid h-11 w-11 place-items-center rounded-2xl bg-sand/20 text-[#c8761b]">
                  <Glyph k={rw.iconKey} size={22} />
                </div>
                <div className="mt-1.5 text-xs font-bold leading-tight text-ink">{rw.title}</div>
                <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-sea/10">
                  <div className="h-full rounded-full bg-gradient-to-r from-sea to-leaf" style={{ width: `${rw.progress}%` }} />
                </div>
              </Card>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}

function UploadBtn({
  icon, label, tone = 'sea', onFile,
}: {
  icon: ReactNode; label: string; tone?: 'sea' | 'leaf';
  onFile?: (file: File) => void;
}) {
  const border = tone === 'leaf' ? 'border-leaf/40' : 'border-sea/40'
  const ref = useRef<HTMLInputElement>(null)
  const [preview, setPreview] = useState<string | null>(null)
  return (
    <button
      type="button"
      onClick={() => ref.current?.click()}
      className={`relative flex min-h-[72px] w-full items-center justify-center gap-2 overflow-hidden rounded-2xl border border-dashed ${border} bg-white/50 py-3 text-sm font-bold text-deep`}
    >
      {preview ? (
        <img src={preview} alt="" className="absolute inset-0 h-full w-full object-cover" />
      ) : (
        <>{icon} {label}</>
      )}
      <input
        ref={ref}
        type="file"
        accept="image/*"
        className="sr-only"
        onChange={(e) => {
          const file = e.target.files?.[0]
          if (!file) return
          setPreview(URL.createObjectURL(file))
          onFile?.(file)
        }}
      />
    </button>
  )
}
