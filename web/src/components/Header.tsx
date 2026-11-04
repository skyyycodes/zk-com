import type { RegistryState } from '../types'

const NAV = [
  ['/funds', 'Funds'],
  ['/list', 'List your fund'],
] as const

export function Header({ state, error, route }: { state: RegistryState | null; error: string | null; route: string }) {
  const section = route.startsWith('/fund/') ? '/funds' : route
  return (
    <header className="topbar">
      <a className="logo" href="#/">
        <span className="mark" aria-hidden="true" />Z<span>-</span>Solvency
      </a>
      <nav className="nav" aria-label="Registry">
        {NAV.map(([path, label]) => (
          <a key={path} href={`#${path}`} aria-current={section === path ? 'page' : undefined}>
            {label}
          </a>
        ))}
      </nav>
      <div className={`chain ${error ? 'offline' : ''}`}>
        {error
          ? 'registry unreachable'
          : state
            ? `${state.network} · ${state.pool} pool · tip ${state.tip}`
            : 'connecting…'}
      </div>
    </header>
  )
}
