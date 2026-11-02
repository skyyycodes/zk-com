export const COIN = 100_000_000

export function zec(zat: number): string {
  return (zat / COIN).toLocaleString('en-US', { maximumFractionDigits: 8 })
}

export function usd(zat: number, zecUsd: number): string {
  return Math.round((zat / COIN) * zecUsd).toLocaleString('en-US')
}

export function short(hex: string | null | undefined): string {
  if (!hex) return ''
  return hex.length > 16 ? `${hex.slice(0, 10)}…${hex.slice(-6)}` : hex
}

export function blocks(n: number): string {
  return `${n} block${n === 1 ? '' : 's'}`
}

/** Zatoshi from a typed ZEC amount ("12.5"), without floating point; null if malformed. */
export function parseZec(s: string): number | null {
  const m = s.trim().match(/^(\d+)(?:\.(\d{1,8}))?$/)
  if (!m) return null
  const zat = Number(m[1]) * COIN + Number((m[2] ?? '').padEnd(8, '0'))
  return Number.isSafeInteger(zat) ? zat : null
}

const HOUR = 48
const DAY = 1152

/** Chain time in words, as the registry writes it: blocks, then hours, then days (75 s blocks). */
export function span(n: number): string {
  if (n < HOUR) return blocks(n)
  if (n < 2 * DAY) return plural(Math.floor(n / HOUR), 'hour')
  return plural(Math.floor(n / DAY), 'day')
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? '' : 's'}`
}

const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' })

/** "2 hours ago" from Unix seconds. */
export function ago(unixSeconds: string | number): string {
  const s = Number(unixSeconds) - Date.now() / 1000
  const abs = Math.abs(s)
  if (abs < 60) return 'just now'
  if (abs < 3600) return rtf.format(Math.round(s / 60), 'minute')
  if (abs < 86400) return rtf.format(Math.round(s / 3600), 'hour')
  return rtf.format(Math.round(s / 86400), 'day')
}
