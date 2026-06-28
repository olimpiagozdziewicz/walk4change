import { useEffect, useRef, useState } from 'react'
import { motion } from 'motion/react'
import { Camera, MapPin, PaperPlaneTilt, CheckCircle, Warning, Sparkle, Leaf, X } from '@phosphor-icons/react'
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
  const [desc, setDesc] = useState('')
  const [photos, setPhotos] = useState<string[]>([])
  const fileRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    api.getEcoReports().then(setReports)
    api.getRewards().then(setRewards)
  }, [])

  const switchTab = (t: Tab) => {
    setTab(t)
    setSent(false)
    setDesc('')
    setPhotos([])
  }

  const onFile = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files || [])
    files.forEach((f) => {
      const r = new FileReader()
      r.onload = () => setPhotos((p) => [...p, String(r.result)])
      r.readAsDataURL(f)
    })
    e.target.value = ''
  }

  const removePhoto = (i: number) => setPhotos((p) => p.filter((_, idx) => idx !== i))

  const PhotoStrip = ({ tone = 'sea' }: { tone?: 'sea' | 'leaf' }) => (
    <div>
      <input ref={fileRef} type="file" accept="image/*" multiple className="hidden" onChange={onFile} />
      <div className="no-scrollbar flex gap-2 overflow-x-auto">
        <button
          type="button"
          onClick={() => fileRef.current?.click()}
          className={`grid h-16 w-16 shrink-0 place-items-center rounded-2xl border border-dashed bg-white/50 text-deep transition active:scale-95 ${tone === 'leaf' ? 'border-leaf/40' : 'border-sea/40'}`}
        >
          <Camera size={20} />
        </button>
        {photos.map((p, i) => (
          <div key={i} className="relative h-16 w-16 shrink-0">
            <img src={p} alt="" className="h-16 w-16 rounded-2xl object-cover" />
            <button
              type="button"
              onClick={() => removePhoto(i)}
              className="absolute -right-1.5 -top-1.5 grid h-5 w-5 place-items-center rounded-full bg-white text-muted shadow"
              aria-label="Usuń"
            >
              <X size={11} weight="bold" />
            </button>
          </div>
        ))}
      </div>
      <p className="mt-1.5 text-xs text-muted">
        {tab === 'done' ? 'Dodaj zdjęcia „przed / po" 📸' : 'Dodaj zdjęcie 📸'}
      </p>
    </div>
  )

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
                  <Pill key={t} tone="muted">
                    {t}
                  </Pill>
                ))}
              </div>
              <textarea
                value={desc}
                onChange={(e) => setDesc(e.target.value)}
                placeholder="Opisz krótko, co widzisz…"
                rows={3}
                className="w-full resize-none rounded-2xl border border-white/70 bg-white/70 px-4 py-3 text-sm text-ink outline-none placeholder:text-muted/70 focus:ring-2 focus:ring-sea/30"
              />
              <PhotoStrip />
              <button type="button" className="flex w-full items-center justify-center gap-2 rounded-2xl border border-dashed border-sea/40 bg-white/50 py-2.5 text-sm font-bold text-deep">
                <MapPin size={18} /> Dodaj lokalizację
              </button>
              <PrimaryButton onClick={() => setSent(true)} className="w-full">
                <PaperPlaneTilt size={18} /> Wyślij zgłoszenie
              </PrimaryButton>
            </div>
          ) : (
            <div className="space-y-3">
              <h2 className="font-display text-lg font-bold text-ink">Pochwal się — posprzątane!</h2>
              <p className="-mt-1 text-sm text-muted">Coś ogarnęłaś sama? Pokaż efekt i zgarnij punkty.</p>
              <div className="flex flex-wrap gap-2">
                {['Plaża', 'Park', 'Las', 'Ulica', 'Brzeg'].map((t) => (
                  <Pill key={t} tone="leaf">
                    {t}
                  </Pill>
                ))}
              </div>
              <textarea
                value={desc}
                onChange={(e) => setDesc(e.target.value)}
                placeholder="Co posprzątałaś? Np. worek śmieci z plaży w Brzeźnie…"
                rows={3}
                className="w-full resize-none rounded-2xl border border-white/70 bg-white/70 px-4 py-3 text-sm text-ink outline-none placeholder:text-muted/70 focus:ring-2 focus:ring-leaf/30"
              />
              <PhotoStrip tone="leaf" />
              <PrimaryButton onClick={() => setSent(true)} className="w-full bg-gradient-to-br from-leaf to-sea">
                <Sparkle size={18} /> Pochwal się
              </PrimaryButton>
            </div>
          )}
        </Card>

        {/* recent activity */}
        <div>
          <h2 className="mb-2 font-display text-lg font-bold text-ink">Ostatnia aktywność</h2>
          <div className="space-y-2.5">
            {reports.map((r) => (
              <Card key={r.id} className="flex items-center gap-3 p-3.5">
                <div className="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-sea/10 text-sea">
                  {r.status === 'cleaned' ? <Sparkle size={18} /> : <Leaf size={18} />}
                </div>
                <div className="flex-1">
                  <div className="text-sm font-bold text-ink">{r.type}</div>
                  <div className="text-xs text-muted">{r.location}</div>
                </div>
                <Pill tone={statusMeta[r.status].tone}>{statusMeta[r.status].label}</Pill>
              </Card>
            ))}
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
