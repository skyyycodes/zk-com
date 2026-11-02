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

/** The route after `#`, e.g. `/fund/mill`; `/` when there is none. */
export function useRoute(): string {
  const [hash, setHash] = useState(() => window.location.hash)
  useEffect(() => {
    const onChange = () => {
      setHash(window.location.hash)
      window.scrollTo(0, 0)
    }
    window.addEventListener('hashchange', onChange)
    return () => window.removeEventListener('hashchange', onChange)
  }, [])
  const route = hash.replace(/^#/, '')
  return route.startsWith('/') ? route : '/'
}

export const fundHref = (id: string) => `#/fund/${encodeURIComponent(id)}`
