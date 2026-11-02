import { useCallback, useEffect, useState } from 'react'
import { useRoute } from '../hooks'
import { ConsoleError, consoleApi, takeToken } from './api'
import { History } from './History'
import { Prove } from './Prove'
import { Register } from './Register'
import { Reserve } from './Reserve'
import { Setup } from './Setup'
import type { Configured, ConsoleState } from './types'
import { Wallet } from './Wallet'

const SCREENS = [
  ['/register', 'Register'],
  ['/reserve', 'Reserve'],
  ['/prove', 'Prove and publish'],
  ['/history', 'History'],
  ['/wallet', 'Wallet'],
] as const

/** Polls the console so balances, the reserve and the registry's view stay current. */
function useConsoleState(enabled: boolean) {
  const [state, setState] = useState<ConsoleState | null>(null)
  const [error, setError] = useState<ConsoleError | Error | null>(null)
  const reload = useCallback(async () => {
    try {
      setState(await consoleApi.state())
      setError(null)
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)))
    }
  }, [])
  useEffect(() => {
    if (!enabled) return
    void reload()
    const t = setInterval(() => void reload(), 4000)
    return () => clearInterval(t)
  }, [enabled, reload])
  return { state, error, reload }
}

/** Where to land: the first step not done yet. */
function nextStep(s: Configured): string {
  if (!s.fund) return '/register'
  if (!s.wallet.reserve || s.wallet.reserve.spent_height != null) return '/reserve'
  return '/prove'
}

export function ConsoleApp() {
  const [token] = useState(() => takeToken())
  const route = useRoute()
  const { state, error, reload } = useConsoleState(token != null)
  const known = SCREENS.some(([path]) => path === route)

  // Name the screen in the address bar, so it stays put while its own state changes.
  useEffect(() => {
    if (state?.configured && !known) window.location.replace(`#${nextStep(state)}`)
  }, [state, known])

  if (!token || (error instanceof ConsoleError && error.status === 401)) {
    return (
      <Shell>
        <section className="card prose">
          <h1>Open the console from its link</h1>
          <p className="desc">
            The fund console holds the fund's keys, so it only answers its own page. Open the link that{' '}
            <code>zsolv console</code> printed in your terminal: it carries this run's access token.
          </p>
        </section>
      </Shell>
    )
  }
  if (!state) {
    return (
      <Shell>
        <div className="empty" role="status">
          {error ? `Can't reach the console (${error.message}). Is zsolv console still running?` : 'Loading…'}
        </div>
      </Shell>
    )
  }
  if (!state.configured) {
    return (
      <Shell>
        <Setup state={state} onDone={reload} />
      </Shell>
    )
  }
  const current = known ? route : nextStep(state)
  return (
    <Shell state={state} current={current}>
      {state.registry_error && (
        <p className="notice" role="alert">
          Registry: {state.registry_error}
        </p>
      )}
      {current === '/register' && <Register state={state} reload={reload} />}
      {current === '/reserve' && <Reserve state={state} reload={reload} />}
      {current === '/prove' && <Prove state={state} reload={reload} />}
      {current === '/history' && <History state={state} />}
      {current === '/wallet' && <Wallet state={state} reload={reload} />}
    </Shell>
  )
}

function Shell({ state, current, children }: { state?: Configured; current?: string; children: React.ReactNode }) {
  return (
    <>
      <header className="topbar">
        <span className="logo">
          Z<span>-</span>Solvency
        </span>
        <span className="tagline">Fund console</span>
        <span className="tag local">Console · local</span>
        {state && (
          <span className="chain">
            {state.network} · tip block {state.tip ?? '…'} · {state.registry_url.replace(/^https?:\/\//, '')}
          </span>
        )}
      </header>
      {state && (
        <nav className="tabs" aria-label="Console">
          {SCREENS.map(([path, label]) => (
            <a key={path} href={`#${path}`} aria-current={current === path ? 'page' : undefined}>
              {label}
            </a>
          ))}
        </nav>
      )}
      <main className="page">{children}</main>
    </>
  )
}
