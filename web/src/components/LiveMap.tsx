/**
 * Żywa mapka spaceru: rzutuje realne punkty GPS (lat/lng) na SVG, auto-dopasowując
 * widok do otrzymanych punktów. Jeden kolorowy ślad + „głowa" na każdego spacerowicza.
 */

export interface MapWalker {
  userId: string
  name: string
  color: string
  trail: { lat: number; lng: number }[]
  isMe: boolean
}

const W = 320
const PAD = 26

export function LiveMap({ walkers, height = 220 }: { walkers: MapWalker[]; height?: number }) {
  const H = height
  const all = walkers.flatMap((w) => w.trail)

  // Auto-fit bounds (with a tiny epsilon so a single point still renders centered).
  const lats = all.map((p) => p.lat)
  const lngs = all.map((p) => p.lng)
  const minLat = lats.length ? Math.min(...lats) : 0
  const maxLat = lats.length ? Math.max(...lats) : 0
  const minLng = lngs.length ? Math.min(...lngs) : 0
  const maxLng = lngs.length ? Math.max(...lngs) : 0
  const spanLat = Math.max(maxLat - minLat, 1e-5)
  const spanLng = Math.max(maxLng - minLng, 1e-5)

  const project = (p: { lat: number; lng: number }): [number, number] => {
    const x = PAD + ((p.lng - minLng) / spanLng) * (W - 2 * PAD)
    // invert lat so north is up
    const y = PAD + ((maxLat - p.lat) / spanLat) * (H - 2 * PAD)
    return [x, y]
  }

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="xMidYMid slice"
      style={{ width: '100%', height: H }}
      className="rounded-2xl"
    >
      <defs>
        <linearGradient id="lm-bg" x1="0" y1="0" x2={W} y2={H}>
          <stop offset="0" stopColor="#dff4f1" />
          <stop offset="1" stopColor="#eaf8fa" />
        </linearGradient>
      </defs>
      <rect width={W} height={H} fill="url(#lm-bg)" />
      {/* nature-zone tint (the whole demo area sits inside the Brzeźno zone) */}
      <rect x="0" y="0" width={W} height={H} fill="rgba(88,184,108,0.10)" />
      <text x={W - 10} y="18" textAnchor="end" className="fill-leaf" fontSize="10" fontWeight="700">
        strefa natury ×3
      </text>

      {all.length === 0 && (
        <text x={W / 2} y={H / 2} textAnchor="middle" className="fill-muted" fontSize="12">
          czekam na pierwsze pingi GPS…
        </text>
      )}

      {walkers.map((w) => {
        if (w.trail.length === 0) return null
        const pts = w.trail.map(project)
        const d = pts.map(([x, y], i) => `${i === 0 ? 'M' : 'L'} ${x.toFixed(1)} ${y.toFixed(1)}`).join(' ')
        const [hx, hy] = pts[pts.length - 1]
        const [sx, sy] = pts[0]
        return (
          <g key={w.userId}>
            <path d={d} fill="none" stroke={w.color} strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" opacity="0.85" />
            <circle cx={sx} cy={sy} r="4" fill="#fff" stroke={w.color} strokeWidth="2" />
            <circle cx={hx} cy={hy} r="7" fill={w.color} stroke="#fff" strokeWidth="2.5" />
            <text x={hx} y={hy - 11} textAnchor="middle" fontSize="11" fontWeight="700" fill={w.color}>
              {w.name}{w.isMe ? ' (Ty)' : ''}
            </text>
          </g>
        )
      })}
    </svg>
  )
}
