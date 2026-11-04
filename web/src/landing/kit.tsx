export const prefersReducedMotion = () => window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false

const PATHS = {
  arrowRight: ['M5 12h14', 'M12 5l7 7-7 7'],
  arrowUpRight: ['M7 17 17 7', 'M7 7h10v10'],
  menu: ['M4 6h16', 'M4 12h16', 'M4 18h16'],
  x: ['M18 6 6 18', 'M6 6l12 12'],
  eyeOff: ['M9.9 4.2A9 9 0 0 1 12 4c7 0 10 8 10 8a17 17 0 0 1-2.2 3.2', 'M6.6 6.6A17 17 0 0 0 2 12s3 8 10 8a9.7 9.7 0 0 0 5.4-1.6', 'M14.1 14.1a3 3 0 1 1-4.2-4.2', 'M2 2l20 20'],
  shield: ['M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z'],
  fileCheck: ['M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z', 'M14 2v6h6', 'M9 15l2 2 4-4'],
  lock: ['M5 11h14v11H5z', 'M8 11V7a4 4 0 0 1 8 0v4'],
} as const

export function Icon({ name, size = 16 }: { name: keyof typeof PATHS; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" className={name === 'arrowRight' ? 'arrow' : undefined}>
      {PATHS[name].map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  )
}

