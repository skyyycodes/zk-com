import { FundPage } from '../components/FundPage'
import { Home } from '../components/Home'
import { ListFund } from '../components/ListFund'
import { useRegistry, useRoute } from '../hooks'
import { Landing } from '../landing/Landing'
import { SiteNav } from '../landing/SiteNav'

export function RegistryApp() {
  const { state, error } = useRegistry()
  const route = useRoute()
  // "/?to=how" is the landing page, scrolled to a section.
  const [path, query = ''] = route.split('?')
  const fundId = path.match(/^\/fund\/(.+)$/)?.[1]

  // The landing page explains the idea on its own, so it renders before the registry answers.
  if (path === '/') {
    return (
      <>
        <SiteNav state={state} error={error} route={route} />
        <main>
          <Landing state={state} to={new URLSearchParams(query).get('to')} />
        </main>
      </>
    )
  }

  let body
  if (path === '/list') {
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
      <SiteNav state={state} error={error} route={route} />
      <main className="page under-nav">{body}</main>
    </>
  )
}
