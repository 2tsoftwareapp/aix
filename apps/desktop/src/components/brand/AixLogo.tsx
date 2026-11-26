export function AixLogo({ size = 30 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" aria-hidden="true">
      <defs>
        <linearGradient id="aix-tile" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#4f46e5" />
          <stop offset="0.55" stopColor="#6d28d9" />
          <stop offset="1" stopColor="#7e22ce" />
        </linearGradient>
        <radialGradient id="aix-sheen" cx="0.5" cy="0" r="1">
          <stop offset="0" stopColor="#ffffff" stopOpacity="0.22" />
          <stop offset="0.5" stopColor="#ffffff" stopOpacity="0" />
        </radialGradient>
      </defs>
      <rect x="32" y="32" width="960" height="960" rx="220" fill="url(#aix-tile)" />
      <rect x="32" y="32" width="960" height="960" rx="220" fill="url(#aix-sheen)" />
      <rect x="33.5" y="33.5" width="957" height="957" rx="218.5" fill="none" stroke="#ffffff" strokeOpacity="0.14" strokeWidth="3" />
      <g transform="translate(512,512)">
        <g stroke="#ffffff" strokeWidth="72" strokeLinecap="round">
          <line x1="-200" y1="-200" x2="200" y2="200" />
          <line x1="200" y1="-200" x2="-200" y2="200" />
        </g>
        <circle cx="-200" cy="-200" r="52" fill="#ffffff" />
        <circle cx="200" cy="200" r="52" fill="#ffffff" />
        <circle cx="-200" cy="200" r="52" fill="#ffffff" />
        <circle cx="200" cy="-200" r="66" fill="#fbbf24" />
        <circle r="34" fill="#ffffff" />
      </g>
    </svg>
  );
}
