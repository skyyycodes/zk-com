import { useState } from 'react'
import { ago, zec } from '../format'
import { fundHref } from '../hooks'
import type { Fund, RegistryState } from '../types'
import { RegistryLog } from './RegistryLog'
import { StatusDot } from './StatusLight'

export function Home({ state }: { state: RegistryState }) {
  const [query, setQuery] = useState('')
  const q = query.trim().toLowerCase()
  const shown = state.funds.filter((f) => `${f.name} ${f.description}`.toLowerCase().includes(q))

  return (
    <>
      <section className="hero">
        <h1>Funds proving their reserves</h1>
        <p className="desc">
          Each fund proves it holds what it promised, and nobody learns who gave or who got. Open a fund to check its
          proof yourself.
        </p>
      </section>
      <section className="card">
        <label className="sr-only" htmlFor="fund-search">
          Search funds
        </label>
        <input
          id="fund-search"
          className="search"
          type="search"
          placeholder="Search funds"
          autoComplete="off"
          spellCheck={false}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {state.funds.length === 0 ? (
          <p className="muted spaced">No funds yet. The first fund to register shows up here.</p>
        ) : shown.length === 0 ? (
          <p className="muted spaced">No fund matches “{query}”.</p>
        ) : (
          <ul className="fund-rows">
            {shown.map((f) => (
              <li key={f.id}>
                <FundRow fund={f} />
              </li>
            ))}
          </ul>
        )}
        <a className="more" href="#/list">
          Run a fund? List it here →
        </a>
      </section>
      <section className="card">
        <RegistryLog events={state.events} funds={state.funds} />
      </section>
    </>
  )
}

function FundRow({ fund }: { fund: Fund }) {
  const l = fund.latest
  return (
    <a className="fund-row" href={fundHref(fund.id)}>
      <StatusDot status={fund.status} />
      <span className="fund-row-text">
        <span className="fund-name">{fund.name}</span>
        <span className="fund-sub">
          {l
            ? `At least ${zec(l.threshold_zat)} ${fund.unit} · held since block ${l.since_height} · checked ${ago(l.received_at)}`
            : fund.status_text}
        </span>
        {l && fund.status !== 'green' && <span className={`fund-note ${fund.status}`}>{fund.status_text}</span>}
      </span>
      <span className="chev" aria-hidden="true">
        ›
      </span>
    </a>
  )
}
