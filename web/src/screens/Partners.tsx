import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { motion } from 'motion/react'
import { Storefront, MapPin, Ticket, HandHeart, Leaf, CalendarDots, ArrowSquareOut } from '@phosphor-icons/react'
import { ScreenHeader, Card, Pill, PrimaryButton, SoftButton, SoonBadge, DemoBanner } from '../components/ui'
import { SponsorIcon } from '../components/SponsorIcon'
import { api, type Sponsor } from '../lib/api'
import { IGTSF_PARTNER } from '../lib/partners'

export function Partners() {
  const nav = useNavigate()
  const [sponsors, setSponsors] = useState<Sponsor[]>([])

  useEffect(() => {
    api.getSponsors().then(setSponsors)
  }, [])

  return (
    <div>
      <ScreenHeader
        title="Partnerzy"
        icon={<Storefront size={22} />}
        subtitle="Wymieniaj punkty na aktywność w naturze u lokalnych firm."
      />

      <div className="space-y-4 px-5 pt-2">
        {/* Prawdziwy partner (spec 2026-10-06) — nazwa/opis w lib/partners.ts */}
        <Card className="p-4">
          <div className="flex items-center gap-3">
            <div className="grid h-14 w-14 shrink-0 place-items-center rounded-2xl bg-gradient-to-br from-leaf/15 to-sea/12 text-[#2f7a45]">
              <Leaf size={28} />
            </div>
            <div className="min-w-0 flex-1">
              <div className="font-display text-lg font-bold leading-tight text-ink">{IGTSF_PARTNER.name}</div>
              <div className="text-xs font-bold text-muted">{IGTSF_PARTNER.description}</div>
            </div>
            <Pill tone="leaf">partner</Pill>
          </div>
          <div className="mt-3 flex flex-wrap gap-2">
            <SoftButton onClick={() => nav('/events')} className="flex-1 py-2.5 text-sm">
              <CalendarDots size={16} /> Wydarzenia
            </SoftButton>
            <a
              href={IGTSF_PARTNER.siteUrl}
              target="_blank"
              rel="noopener"
              className="inline-flex flex-1 items-center justify-center gap-2 rounded-2xl border border-white/70 bg-white/80 px-5 py-2.5 text-sm font-bold text-deep transition active:scale-[0.97]"
            >
              igtsf.com <ArrowSquareOut size={14} />
            </a>
          </div>
        </Card>

        <DemoBanner>
          Program partnerski w przygotowaniu. Tu lokalne firmy — kawiarnie, wypożyczalnie — będą nagradzać
          spacerowiczów. Poniżej przykłady, jak to będzie wyglądać. Chcesz być partnerem? Napisz do nas.
        </DemoBanner>

        <Card className="bg-gradient-to-br from-sea/10 to-leaf/10 p-4">
          <p className="text-sm font-semibold leading-snug text-deep">
            🌿 Każda wymiana wspiera lokalny biznes i wyciąga Cię na świeże powietrze — kajak, SUP, rower, rejs.
          </p>
        </Card>

        <div className="space-y-3">
          {sponsors.map((s, i) => (
            <motion.div key={s.id} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.06 }}>
              <Card className="p-4">
                <div className="flex items-center gap-3">
                  <div className="grid h-14 w-14 shrink-0 place-items-center rounded-2xl bg-gradient-to-br from-sea/12 to-leaf/15 text-sea">
                    <SponsorIcon keyName={s.iconKey} size={28} />
                  </div>
                  <div className="flex-1">
                    <div className="font-display text-lg font-bold leading-tight text-ink">{s.name}</div>
                    <div className="text-xs font-bold text-muted">{s.category}</div>
                    <div className="mt-0.5 inline-flex items-center gap-1 text-xs font-bold text-muted">
                      <MapPin size={12} /> {s.place}
                    </div>
                  </div>
                  <SoonBadge />
                </div>
                <div className="mt-3 flex items-center justify-between gap-2">
                  <Pill tone="leaf">
                    <Ticket size={12} /> {s.offer}
                  </Pill>
                  <span className="text-xs font-bold text-deep">{s.pointsCost} pkt</span>
                </div>
                <PrimaryButton disabled className="mt-3 w-full py-2.5 text-sm">
                  Wkrótce
                </PrimaryButton>
              </Card>
            </motion.div>
          ))}
        </div>

        {/* sponsor acquisition */}
        <Card className="p-5 text-center">
          <div className="mx-auto mb-2 grid h-12 w-12 place-items-center rounded-2xl bg-sand/20 text-sea">
            <HandHeart size={26} />
          </div>
          <h2 className="font-display text-lg font-bold text-ink">Masz lokalny biznes?</h2>
          <p className="mx-auto mt-1 max-w-[280px] text-sm text-muted">
            Zostań partnerem SeaSteps — docieraj do aktywnych ludzi nad Bałtykiem i wspieraj zdrowy, eko styl życia.
            Napisz do nas, chętnie pogadamy.
          </p>
        </Card>
      </div>
    </div>
  )
}
