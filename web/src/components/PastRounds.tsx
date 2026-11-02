import { ago } from '../format'
import type { Fund, PastItem } from '../types'

/** A strip of the fund's rounds and refusals, oldest first. */
export function PastRounds({ fund }: { fund: Fund }) {
  if (fund.past.length === 0) return null
  return (
    <section className="card">
      <h2 className="eyebrow">Past rounds</h2>
      <ol className="strip">
        {fund.past.map((p, i) => (
          <li key={`${p.time}-${i}`} title={`${p.text} (${ago(p.time)})`}>
            <span className={`dot ${p.light}`} aria-hidden="true" />
            <span className="strip-label">{p.accepted ? `R${p.round}` : 'refused'}</span>
            <span className="sr-only">
              {p.accepted ? `Round ${p.round}` : 'Refused'}: {p.text}
            </span>
          </li>
        ))}
      </ol>
      <p className="muted small">{summary(fund.past)}</p>
    </section>
  )
}

function summary(past: PastItem[]): string {
  const rounds = past.filter((p) => p.accepted)
  const refused = past.length - rounds.length
  const parts: string[] = []
  if (rounds.length > 0) {
    const firstGreenRun = rounds.findIndex((_, i) => rounds.slice(i).every((r) => r.light === 'green'))
    if (firstGreenRun === 0) parts.push(`Green every round.`)
    else if (firstGreenRun > 0) parts.push(`Amber while the reserve built up its holding time, green since round ${rounds[firstGreenRun].round}.`)
    else parts.push(`Latest round: ${rounds[rounds.length - 1].text}.`)
  }
  if (refused > 0) parts.push(`${refused} submission${refused === 1 ? '' : 's'} refused.`)
  return parts.join(' ')
}
