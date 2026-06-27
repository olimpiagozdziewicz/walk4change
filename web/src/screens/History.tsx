import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { motion } from 'motion/react'
import { Clock, Footprints, MapPin, Leaf, UsersThree, Plus, MapTrifold, Star, PawPrint } from '@phosphor-icons/react'
import { ScreenHeader, Card, Pill, PrimaryButton } from '../components/ui'
import { RouteMap } from '../components/RouteMap'
import { getWalks, type SavedWalk } from '../lib/walks'

function fmt(sec: number) {
  const m = Math.floor(sec / 60)
  return `${m} min`
}

function Photo({ src }: { src: string }) {
  const isImg = src.startsWith('data:') || src.startsWith('http')
  if (isImg) return <img src={src} alt="" className="h-16 w-16 shrink-0 rounded-2xl object-cover" />
  return (
    <div className="grid h-16 w-16 shrink-0 place-items-center rounded-2xl bg-gradient-to-br from-sea/15 to-leaf/15 text-2xl">
      {src}
    </div>
  )
}

export function History() {
  const nav = useNavigate()
  const [walks, setWalks] = useState<SavedWalk[]>([])

  useEffect(() => {
    setWalks(getWalks())
  }, [])

  // auto-ulubione: miejsca odwiedzone ≥2 razy
  const counts = walks.reduce<Record<string, number>>((m, w) => {
    m[w.place] = (m[w.place] || 0) + 1
    return m
  }, {})
  const favorites = Object.keys(counts).filter((p) => counts[p] >= 2)
  const favSet = new Set(favorites)

  return (
    <div>
      <ScreenHeader title="Moje spacery" icon={<MapTrifold size={22} />} subtitle="Twoje trasy, punkty i zdjęcia z drogi." />

      <div className="space-y-4 px-5 pt-2">
        <PrimaryButton onClick={() => nav('/walk')} className="w-full">
          <Plus size={18} /> Nowy spacer
        </PrimaryButton>

        {favorites.length > 0 && (
          <div>
            <h2 className="mb-2 flex items-center gap-2 font-display text-base font-bold text-ink">
              <Star size={16} weight="fill" className="text-sand" /> Ulubione miejsca
            </h2>
            <div className="flex flex-wrap gap-2">
              {favorites.map((p) => (
                <span key={p} className="inline-flex items-center gap-1.5 rounded-full bg-sand/20 px-3 py-1.5 text-xs font-bold text-[#8a6418]">
                  <Star size={12} weight="fill" /> {p}
                </span>
              ))}
            </div>
          </div>
        )}

        {walks.map((w, i) => (
          <motion.div key={w.id} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.07 }}>
            <Card className="overflow-hidden">
              <RouteMap seed={w.routeSeed} height={150} />
              <div className="p-4">
                <div className="flex items-start justify-between gap-2">
                  <div>
                    <div className="flex items-center gap-1.5 font-display text-lg font-bold leading-tight text-ink">
                      {w.place}
                      {favSet.has(w.place) && <Star size={15} weight="fill" className="text-sand" />}
                    </div>
                    <div className="text-xs font-bold text-muted">{w.dateLabel}</div>
                  </div>
                  <span className="font-display text-xl font-bold text-sea">+{w.points}</span>
                </div>

                <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-xs font-bold text-muted">
                  <span className="inline-flex items-center gap-1">
                    <Clock size={13} /> {fmt(w.durationSec)}
                  </span>
                  <span className="inline-flex items-center gap-1">
                    <Footprints size={13} /> {w.steps.toLocaleString('pl-PL')} kroków
                  </span>
                  <span className="inline-flex items-center gap-1">
                    <MapPin size={13} /> trasa zapisana
                  </span>
                </div>

                {(w.inNature || w.withSomeone || w.withDog) && (
                  <div className="mt-2.5 flex flex-wrap gap-1.5">
                    {w.inNature && (
                      <Pill tone="leaf">
                        <Leaf size={12} /> natura ×3
                      </Pill>
                    )}
                    {w.withSomeone && (
                      <Pill tone="sea">
                        <UsersThree size={12} /> z kimś ×1.5
                      </Pill>
                    )}
                    {w.withDog && (
                      <Pill tone="sand">
                        <PawPrint size={12} /> z psem
                      </Pill>
                    )}
                  </div>
                )}

                {w.photos.length > 0 && (
                  <div className="no-scrollbar mt-3 flex gap-2 overflow-x-auto">
                    {w.photos.map((p, idx) => (
                      <Photo key={idx} src={p} />
                    ))}
                  </div>
                )}
              </div>
            </Card>
          </motion.div>
        ))}
      </div>
    </div>
  )
}
