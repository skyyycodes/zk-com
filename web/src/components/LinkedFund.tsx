import { useFund } from '../hooks'
import { FundPage } from './FundPage'

/** The fund behind a share link, polled so new rounds appear. */
export function LinkedFund({ link }: { link: string }) {
  const { fund, error, missing } = useFund(link)
  if (missing) {
    return (
      <section className="card prose">
        <h1>No fund at this link</h1>
        <p className="desc">
          Check that you copied the whole link. If it came from a fund, ask them to send it again: the link stays the
          same every round.
        </p>
        <a className="btn" href="#/verify">
          Paste another link
        </a>
      </section>
    )
  }
  if (!fund) {
    return (
      <div className="empty" role="status">
        {error ? `Can't reach the registry (${error}). Trying again every few seconds.` : 'Loading the fund…'}
      </div>
    )
  }
  // key: a different fund starts with a fresh verify panel
  return <FundPage key={link} fund={fund} link={link} />
}
