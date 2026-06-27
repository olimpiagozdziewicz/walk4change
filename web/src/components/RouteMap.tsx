function rng(seed: number) {
  let s = seed >>> 0
  return () => {
    s = (s + 0x6d2b79f5) | 0
    let t = Math.imul(s ^ (s >>> 15), 1 | s)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

/**
 * Stylizowana mapka trasy generowana deterministycznie z `seed`.
 * Docelowo: render z realnych punktów GPS z backendu.
 */
export function RouteMap({ seed, className = '', height = 150 }: { seed: number; className?: string; height?: number }) {
  const W = 300
  const H = height
  const rand = rng(seed)
  const pts: [number, number][] = []
  const n = 5 + Math.floor(rand() * 3)
  for (let i = 0; i < n; i++) {
    const x = 24 + (i / (n - 1)) * (W - 48)
    const y = 28 + rand() * (H - 70)
    pts.push([x, y])
  }
  // smooth path (Catmull-Rom -> Bezier)
  let d = `M ${pts[0][0]} ${pts[0][1]}`
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[i === 0 ? 0 : i - 1]
    const p1 = pts[i]
    const p2 = pts[i + 1]
    const p3 = pts[i + 2] ?? p2
    const c1x = p1[0] + (p2[0] - p0[0]) / 6
    const c1y = p1[1] + (p2[1] - p0[1]) / 6
    const c2x = p2[0] - (p3[0] - p1[0]) / 6
    const c2y = p2[1] - (p3[1] - p1[1]) / 6
    d += ` C ${c1x} ${c1y} ${c2x} ${c2y} ${p2[0]} ${p2[1]}`
  }
  const start = pts[0]
  const end = pts[pts.length - 1]
  const gid = `rm-${seed}`

  return (
    <svg viewBox={`0 0 ${W} ${H}`} className={className} preserveAspectRatio="xMidYMid slice" style={{ width: '100%', height }}>
      <defs>
        <linearGradient id={`${gid}-bg`} x1="0" y1="0" x2={W} y2={H}>
          <stop offset="0" stopColor="#dff4f1" />
          <stop offset="1" stopColor="#eaf8fa" />
        </linearGradient>
        <linearGradient id={`${gid}-line`} x1="0" y1="0" x2={W} y2="0">
          <stop offset="0" stopColor="#0f8b8d" />
          <stop offset="1" stopColor="#58b86c" />
        </linearGradient>
      </defs>
      {/* sea base */}
      <rect width={W} height={H} fill={`url(#${gid}-bg)`} />
      {/* coastline / sand */}
      <path d={`M0 ${H - 18} Q ${W * 0.3} ${H - 34} ${W * 0.6} ${H - 20} T ${W} ${H - 24} V ${H} H 0 Z`} fill="rgba(244,201,115,0.55)" />
      {/* nature blobs */}
      <circle cx={W * 0.18} cy={H * 0.3} r="10" fill="rgba(88,184,108,0.22)" />
      <circle cx={W * 0.8} cy={H * 0.5} r="14" fill="rgba(88,184,108,0.18)" />
      {/* route */}
      <path d={d} fill="none" stroke={`url(#${gid}-line)`} strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" strokeDasharray="1 9" opacity="0.9" />
      <path d={d} fill="none" stroke={`url(#${gid}-line)`} strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" />
      {/* start / end */}
      <circle cx={start[0]} cy={start[1]} r="6" fill="#58b86c" stroke="#fff" strokeWidth="2.5" />
      <circle cx={end[0]} cy={end[1]} r="6" fill="#0f8b8d" stroke="#fff" strokeWidth="2.5" />
    </svg>
  )
}
