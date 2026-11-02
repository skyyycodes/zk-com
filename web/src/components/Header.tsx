import type { RegistryState } from '../types'

export function Header({ state, error }: { state: RegistryState | null; error: string | null }) {
  return (
    <header className="topbar">
      <a className="logo" href="#/">
        Z<span>-</span>Solvency
      </a>
      <div className="tagline">Show the money, not the people.</div>
      <div className={`chain ${error ? 'offline' : ''}`}>
        {error
          ? 'registry unreachable'
          : state
            ? `${state.network} · ${state.pool} pool · tip block ${state.tip}`
            : 'connecting…'}
      </div>
    </header>
  )
}
