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
