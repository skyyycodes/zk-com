import { useEffect, useRef, useState } from 'react'

export const prefersReducedMotion = () => window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false

/** True once the element has scrolled into view (and stays true). */
export function useInView<T extends Element>(threshold = 0.15) {
  const ref = useRef<T>(null)
  const [seen, setSeen] = useState(prefersReducedMotion)
  useEffect(() => {
    if (seen || !ref.current) return
    const io = new IntersectionObserver(([e]) => e.isIntersecting && setSeen(true), { threshold })
    io.observe(ref.current)
    return () => io.disconnect()
  }, [seen, threshold])
  return [ref, seen] as const
}

/** The template's section label: a short rule, then mono text. */
export function Label({ children, inverted }: { children: React.ReactNode; inverted?: boolean }) {
  return <span className={`l-label ${inverted ? 'inv' : ''}`}>{children}</span>
}

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

type Pt = { x: number; y: number; z: number }
const rotY = (p: Pt, a: number): Pt => ({ x: p.x * Math.cos(a) - p.z * Math.sin(a), y: p.y, z: p.x * Math.sin(a) + p.z * Math.cos(a) })
const rotX = (p: Pt, a: number): Pt => ({ x: p.x, y: p.y * Math.cos(a) - p.z * Math.sin(a), z: p.y * Math.sin(a) + p.z * Math.cos(a) })
const rotZ = (p: Pt, a: number): Pt => ({ x: p.x * Math.cos(a) - p.y * Math.sin(a), y: p.x * Math.sin(a) + p.y * Math.cos(a), z: p.z })

const BLOCKS = '░▒▓█▀▄▌▐│─┤├┴┬╭╮╰╯'
const RINGS = '·∘○◯◌●◉'

/** ASCII shapes drawn on a canvas, in the page's ink colour. Still for reduced motion. */
export function AsciiCanvas({ shape, className }: { shape: 'sphere' | 'tetra' | 'wave'; className?: string }) {
  const ref = useRef<HTMLCanvasElement>(null)
  useEffect(() => {
    const canvas = ref.current
    const ctx = canvas?.getContext('2d')
    if (!canvas || !ctx) return
    const still = prefersReducedMotion()
    let frame = 0
    let time = 0.6
    let ink = '23,20,17'

    const resize = () => {
      const dpr = window.devicePixelRatio || 1
      const r = canvas.getBoundingClientRect()
      canvas.width = r.width * dpr
      canvas.height = r.height * dpr
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      const c = getComputedStyle(canvas).color.match(/\d+(\.\d+)?/g)
      if (c) ink = c.slice(0, 3).join(',')
    }

    const draw = () => {
      const r = canvas.getBoundingClientRect()
      ctx.clearRect(0, 0, r.width, r.height)
      ctx.textAlign = 'center'
      ctx.textBaseline = 'middle'
      const cx = r.width / 2
      const cy = r.height / 2
      const pts: (Pt & { ch: string })[] = []

      if (shape === 'wave') {
        ctx.font = '14px monospace'
        const cols = Math.floor(r.width / 20)
        const rows = Math.floor(r.height / 20)
        for (let y = 0; y < rows; y++)
          for (let x = 0; x < cols; x++) {
            const w = (Math.sin(x * 0.2 + time * 2) * Math.cos(y * 0.15 + time) + Math.sin((x + y) * 0.1 + time * 1.5) + Math.cos(x * 0.1 - y * 0.1 + time * 0.8)) / 3
            const n = (w + 1) / 2
            ctx.fillStyle = `rgba(${ink},${0.15 + n * 0.5})`
            ctx.fillText(RINGS[Math.floor(n * (RINGS.length - 1))], (x + 0.5) * (r.width / cols), (y + 0.5) * (r.height / rows))
          }
      } else if (shape === 'sphere') {
        ctx.font = '12px monospace'
        const rad = Math.min(r.width, r.height) * 0.45
        for (let phi = 0; phi < Math.PI * 2; phi += 0.15)
          for (let th = 0; th < Math.PI; th += 0.15) {
            let p: Pt = { x: Math.sin(th) * Math.cos(phi + time * 0.5), y: Math.sin(th) * Math.sin(phi + time * 0.5), z: Math.cos(th) }
            p = rotX(rotY(p, time * 0.3), time * 0.2)
            pts.push({ x: cx + p.x * rad, y: cy + p.y * rad, z: p.z, ch: BLOCKS[Math.floor(((p.z + 1) / 2) * (BLOCKS.length - 1))] })
          }
      } else {
        ctx.font = '18px monospace'
        const scale = Math.min(r.width, r.height) * 0.6
        const v: Pt[] = [{ x: 0, y: 1, z: 0 }, { x: -0.943, y: -0.333, z: -0.5 }, { x: 0.943, y: -0.333, z: -0.5 }, { x: 0, y: -0.333, z: 1 }]
        const put = (p: Pt) => {
          p = rotZ(rotX(rotY(p, time * 0.4), time * 0.3), time * 0.2)
          const d = (p.z + 1.5) / 3
          pts.push({ x: cx + p.x * scale, y: cy - p.y * scale, z: p.z, ch: BLOCKS[Math.min(Math.floor(d * (BLOCKS.length - 1)), BLOCKS.length - 1)] })
        }
        for (const [i, j] of [[0, 1], [0, 2], [0, 3], [1, 2], [2, 3], [3, 1]])
          for (let t = 0; t <= 1; t += 0.05) put({ x: v[i].x + (v[j].x - v[i].x) * t, y: v[i].y + (v[j].y - v[i].y) * t, z: v[i].z + (v[j].z - v[i].z) * t })
        for (const [i, j, k] of [[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]])
          for (let u = 0; u <= 1; u += 0.12)
            for (let w = 0; w <= 1 - u; w += 0.12) {
              const q = 1 - u - w
              put({ x: v[i].x * u + v[j].x * w + v[k].x * q, y: v[i].y * u + v[j].y * w + v[k].y * q, z: v[i].z * u + v[j].z * w + v[k].z * q })
            }
      }

      pts.sort((a, b) => a.z - b.z)
      for (const p of pts) {
        ctx.fillStyle = `rgba(${ink},${0.2 + (p.z + 1) * 0.4})`
        ctx.fillText(p.ch, p.x, p.y)
      }
      if (still) return
      time += shape === 'wave' ? 0.03 : 0.02
      frame = requestAnimationFrame(draw)
    }

    resize()
    draw()
    const onResize = () => {
      resize()
      if (still) draw()
    }
    window.addEventListener('resize', onResize)
    return () => {
      window.removeEventListener('resize', onResize)
      cancelAnimationFrame(frame)
    }
  }, [shape])
  return <canvas ref={ref} className={`ascii ${className ?? ''}`} aria-hidden="true" />
}
