import { FundList } from './components/FundList'
import { FundPage } from './components/FundPage'
import { Header } from './components/Header'
import { RegistryLog } from './components/RegistryLog'
import { useRegistry, useSelectedFundId } from './hooks'

export default function App() {
  const { state, error } = useRegistry()
  const routeId = useSelectedFundId()

  if (!state) {
    return (
      <>
        <Header state={null} error={error} />
        <div className="empty">{error ? `Can't reach the registry: ${error}` : 'Loading registry…'}</div>
      </>
    )
  }

  const selectedId = routeId ?? state.funds[0]?.id ?? null
  const fund = state.funds.find((f) => f.id === selectedId) ?? null

  return (
    <>
      <Header state={state} error={error} />
      <div className="layout">
        <aside>
          <FundList funds={state.funds} selectedId={selectedId} />
          <RegistryLog events={state.events} funds={state.funds} />
        </aside>
        <main>
          {fund ? (
            // key: a different fund starts with a fresh verify panel
            <FundPage key={fund.id} fund={fund} />
          ) : (
            <div className="empty">{state.funds.length ? 'No such fund in this registry.' : 'No funds registered yet.'}</div>
          )}
        </main>
      </div>
    </>
  )
}
