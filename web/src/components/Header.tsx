import type { RegistryState } from '../types'

export function Header({ state, error }: { state: RegistryState | null; error: string | null }) {
  return (
    <header className="topbar">
      <div className="logo">
        Z<span>-</span>Solvency
      </div>
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
