import { useCallback, useEffect, useState } from 'react'
import { api } from './api'
import type { Fund, RegistryState } from './types'

/** Polls the registry's chain facts (tip, network). */
export function useRegistry(intervalMs = 10000) {
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

/** Polls one fund through its share link, so a new round shows up live. */
export function useFund(link: string, intervalMs = 5000) {
  const [fund, setFund] = useState<Fund | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [missing, setMissing] = useState(false)

  useEffect(() => {
    let live = true
    const load = async () => {
      try {
        const f = await api.fund(link)
        if (live) {
          setFund(f)
          setError(null)
        }
      } catch (e) {
        const m = e instanceof Error ? e.message : String(e)
        if (live) {
          if (m.includes('no fund at this link')) setMissing(true)
          else setError(m)
        }
      }
    }
    void load()
    const timer = setInterval(() => void load(), intervalMs)
    return () => {
      live = false
      clearInterval(timer)
    }
  }, [link, intervalMs])

  return { fund, error, missing }
}

/** The route after `#`, e.g. `/v/<token>`; `/` when there is none. */
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

export const linkHref = (link: string) => `#/v/${link}`
