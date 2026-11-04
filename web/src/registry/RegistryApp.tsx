import { FundPage } from '../components/FundPage'
import { Header } from '../components/Header'
import { Home } from '../components/Home'
import { Landing } from '../components/Landing'
import { ListFund } from '../components/ListFund'
import { useRegistry, useRoute } from '../hooks'

export function RegistryApp() {
  const { state, error } = useRegistry()
  const route = useRoute()
  const fundId = route.match(/^\/fund\/(.+)$/)?.[1]

  // The landing page explains the idea on its own, so it renders before the registry answers.
  if (route === '/') {
    return (
      <>
        <Header state={state} error={error} route={route} />
        <main>
          <Landing state={state} />
        </main>
      </>
    )
  }

  let body
  if (route === '/list') {
    body = <ListFund />
  } else if (!state) {
    body = (
      <div className="empty" role="status">
        {error ? `Can't reach the registry (${error}). Trying again every few seconds.` : 'Loading registry…'}
      </div>
    )
  } else if (fundId) {
    const fund = state.funds.find((f) => f.id === decodeURIComponent(fundId))
    body = fund ? (
      // key: a different fund starts with a fresh verify panel
      <FundPage key={fund.id} fund={fund} />
    ) : (
      <div className="empty">
        No fund called “{decodeURIComponent(fundId)}” in this registry. <a href="#/funds">See all funds</a>
      </div>
    )
  } else {
    body = <Home state={state} />
  }

  return (
    <>
      <Header state={state} error={error} route={route} />
      <main className="page">{body}</main>
    </>
  )
}
