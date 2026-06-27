export function LogoMark({ size = 40 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" fill="none" aria-hidden>
      <defs>
        <linearGradient id="ssMark" x1="8" y1="8" x2="56" y2="56" gradientUnits="userSpaceOnUse">
          <stop stopColor="#0f8b8d" />
          <stop offset="1" stopColor="#58b86c" />
        </linearGradient>
        <clipPath id="ssClip">
          <rect x="4" y="4" width="56" height="56" rx="18" />
        </clipPath>
      </defs>
      <rect x="4" y="4" width="56" height="56" rx="18" fill="url(#ssMark)" />
      <g clipPath="url(#ssClip)">
        <rect x="0" y="18" width="50" height="13" rx="6.5" fill="#fff" fillOpacity="0.55" transform="rotate(-12 25 24)" />
        <rect x="22" y="40" width="42" height="11" rx="5.5" fill="#fff" fillOpacity="0.5" transform="rotate(12 42 45)" />
      </g>
    </svg>
  )
}

export function Logo() {
  return (
    <div className="flex items-center gap-2.5">
      <LogoMark size={36} />
      <span className="font-display text-[22px] font-bold tracking-tight text-deep">
        Sea<span className="text-sea">Steps</span>
      </span>
    </div>
  )
}
