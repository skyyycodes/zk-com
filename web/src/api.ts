import type { RegistryState, Verdict, ViewingKeyExport } from './types'

export async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, init)
  const body = await res.json().catch(() => null)
  if (!res.ok) {
    const message = body && typeof body === 'object' && 'error' in body ? String(body.error) : `${res.status} ${res.statusText}`
    throw new Error(message)
  }
  return body as T
}

const fundPath = (id: string) => `/api/funds/${encodeURIComponent(id)}`

export const api = {
  mode: () => request<{ mode: 'registry' | 'console' }>('/api/mode'),
  state: () => request<RegistryState>('/api/state'),
  verify: (id: string) => request<Verdict>(`${fundPath(id)}/verify`, { method: 'POST' }),
  viewingKey: async (id: string): Promise<ViewingKeyExport | null> => {
    const res = await fetch(`${fundPath(id)}/viewing-key`)
    return res.ok ? ((await res.json()) as ViewingKeyExport) : null
  },
  attestationUrl: (id: string) => `${fundPath(id)}/attestation`,
}
