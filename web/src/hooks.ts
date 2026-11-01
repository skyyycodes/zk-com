import { useCallback, useEffect, useState } from 'react'
import { api } from './api'
import type { RegistryState } from './types'

/** Polls the registry so new attestations and refusals show up live. */
export function useRegistry(intervalMs = 5000) {
  const [state, setState] = useState<RegistryState | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    try {
      setState(await api.state())
      setError(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void load()
    const timer = setInterval(() => void load(), intervalMs)
    return () => clearInterval(timer)
  }, [load, intervalMs])

  return { state, error }
}

/** `#/fund/<id>` selects a fund; anything else falls back to the first one. */
export function useSelectedFundId(): string | null {
  const [hash, setHash] = useState(() => window.location.hash)
  useEffect(() => {
    const onChange = () => setHash(window.location.hash)
    window.addEventListener('hashchange', onChange)
    return () => window.removeEventListener('hashchange', onChange)
  }, [])
  const match = hash.match(/^#\/?fund\/(.+)$/)
  return match ? decodeURIComponent(match[1]) : null
}

export const fundHref = (id: string) => `#/fund/${encodeURIComponent(id)}`
