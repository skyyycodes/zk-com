import type { Fund, RegistryState, Verdict, ViewingKeyExport } from './types'

export async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, init)
  const body = await res.json().catch(() => null)
  if (!res.ok) {
    const message = body && typeof body === 'object' && 'error' in body ? String(body.error) : `${res.status} ${res.statusText}`
    throw new Error(message)
  }
  return body as T
}

// Funds are unlisted: everything about a fund goes through its share-link token.
const linkPath = (link: string) => `/api/links/${encodeURIComponent(link)}`

export const api = {
  mode: () => request<{ mode: 'registry' | 'console' }>('/api/mode'),
  state: () => request<RegistryState>('/api/state'),
  fund: (link: string) => request<Fund>(linkPath(link)),
  verify: (link: string) => request<Verdict>(`${linkPath(link)}/verify`, { method: 'POST' }),
  viewingKey: async (link: string): Promise<ViewingKeyExport | null> => {
    const res = await fetch(`${linkPath(link)}/viewing-key`)
    return res.ok ? ((await res.json()) as ViewingKeyExport) : null
  },
  attestationUrl: (link: string) => `${linkPath(link)}/attestation`,
}

/** The token from a pasted share link (".../#/v/<token>") or a bare token; null if neither. */
export function linkToken(input: string): string | null {
  const s = input.trim()
  const m = s.match(/#\/v\/([0-9a-f]{32})\b/i) ?? s.match(/^([0-9a-f]{32})$/i)
  return m ? m[1].toLowerCase() : null
}
