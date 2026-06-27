import { motion } from 'motion/react'

const COLORS = ['#0f8b8d', '#58b86c', '#f4c973', '#0c5a71', '#ffffff']

/** Deszcz konfetti w kolorach apki — sukces (koniec spaceru / zgłoszenie eko). */
export function Celebrate({ pieces = 40 }: { pieces?: number }) {
  const arr = Array.from({ length: pieces })
  return (
    <div className="pointer-events-none absolute inset-0 z-50 overflow-hidden">
      {arr.map((_, i) => {
        const left = Math.random() * 100
        const delay = Math.random() * 0.25
        const dur = 1.3 + Math.random() * 0.9
        const w = 6 + Math.random() * 8
        const drift = (Math.random() - 0.5) * 140
        const spin = (Math.random() - 0.5) * 720
        const color = COLORS[i % COLORS.length]
        const round = i % 3 === 0
        return (
          <motion.div
            key={i}
            initial={{ y: '-12%', x: 0, opacity: 0, rotate: 0 }}
            animate={{ y: '115%', x: drift, opacity: [0, 1, 1, 0], rotate: spin }}
            transition={{ duration: dur, delay, ease: 'easeOut' }}
            style={{
              position: 'absolute',
              top: 0,
              left: `${left}%`,
              width: w,
              height: round ? w : w * 0.55,
              background: color,
              borderRadius: round ? '50%' : 2,
            }}
          />
        )
      })}
    </div>
  )
}
