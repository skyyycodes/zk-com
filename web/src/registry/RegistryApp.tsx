import { Create } from '../components/Create'
import { LinkedFund } from '../components/LinkedFund'
import { VerifyLink } from '../components/VerifyLink'
import { useRegistry, useRoute } from '../hooks'
import { Landing } from '../landing/Landing'
import { SiteNav } from '../landing/SiteNav'

/**
 * The public site: the landing page, how to create a fund's link, and the
 * fund behind a link. There is no list of funds; a fund is reached only by
 * the link its treasurer shares.
 */
export function RegistryApp() {
  const { state, error } = useRegistry()
  const route = useRoute()
  // "/?to=how" is the landing page, scrolled to a section.
  const [path, query = ''] = route.split('?')
  const link = path.match(/^\/v\/([0-9a-fA-F]+)$/)?.[1]

  let body
  if (path === '/create' || path === '/list') {
    body = <Create />
  } else if (path === '/verify') {
    body = <VerifyLink />
  } else if (link) {
    body = <LinkedFund link={link.toLowerCase()} />
  } else {
    return (
      <>
        <SiteNav state={state} error={error} route={route} />
        <main>
          <Landing state={state} to={new URLSearchParams(query).get('to')} />
        </main>
      </>
    )
  }

  return (
    <>
      <SiteNav state={state} error={error} route={route} />
      <main className="page under-nav">{body}</main>
    </>
  )
}
