import { useEffect, useState } from 'react'
import { zec } from '../format'
import { fundHref } from '../hooks'
import type { RegistryState } from '../types'
import { StatusDot } from './StatusLight'

// The demo's fictional fund, as its viewing key would show it (scripts/demo.sh).
const LEDGER = [
  { height: 22, who: 'to members', amount: '−30', memo: 'strike pay wk2: S. Haddad, P. Ruiz, +28' },
  { height: 21, who: 'from anonymous', amount: '+25', memo: 'from a manager who agrees' },
  { height: 14, who: 'to members', amount: '−30', memo: 'strike pay wk1: M. Okafor, J. Lindqvist, +28' },
  { height: 13, who: 'from Bakery co-op', amount: '+40', memo: 'bread money' },
  { height: 4, who: "from a nurse at St. Anne's", amount: '+60', memo: 'keep going' },
  { height: 1, who: 'from Dockworkers Local 9', amount: '+150', memo: 'solidarity from the docks' },
]

const CLAUSES = [
  {
    text: 'at least 480 ZEC',
    how: 'The circuit commits to the notes’ total in eighths of a coin, and the fund opens it only as far as the threshold. Over-claiming is impossible in the proof itself.',
    stops: 'Revealing the real balance.',
  },
  {
    text: 'held since block 6',
    how: 'Two proofs over the same notes, one at each height. Their registry nullifiers must match, and a spent note can’t come back.',
    stops: 'Borrowing coins for audit day.',
  },
  {
    text: 'checked at block 32',
    how: 'The signed message names block 32’s hash. If a reorg replaces that block, the proof stops verifying and the light turns red.',
    stops: 'Replaying an old proof.',
  },
  {
    text: 'signed by the fund',
    how: 'A spend-auth signature from the notes’ own key, plus the fund key bound when the fund first registered.',
    stops: 'Proving with someone else’s coins, or publishing as another fund.',
  },
  {
    text: 'counted by no other fund',
    how: 'Each note shows a nullifier scoped to this registry, never its real one. The registry refuses a note another fund already counted.',
    stops: 'Two funds counting the same coins.',
  },
]

const CASES = [
  {
    year: '2021',
    name: '612 Humanitarian Relief Fund, Hong Kong',
    what: 'Paid medical and legal fees for 2019 protesters. Its trustees were arrested in 2022, and six people were convicted of failing to register the fund.',
    lesson: 'Legal-defence funds become targets.',
  },
  {
    year: '2022',
    name: 'Freedom Convoy donors, Canada',
    what: 'A breach of the donation platform exposed 92,845 donors by name.',
    lesson: 'Donor lists leak.',
  },
  {
    year: '2020',
    name: 'Feminist Coalition, Nigeria',
    what: 'During #EndSARS its bank account and payment link were cut. A year later, critics demanded it account for the money.',
    lesson: 'A fund gets cut off, then doubted.',
  },
]

const LIGHTS = [
  { status: 'green', name: 'Green', means: 'The reserve is proven, covers the published obligations, and has been held as long as the fund promised.' },
  { status: 'amber', name: 'Amber', means: 'Proven, with a caveat: the proof is late, the reserve is below the obligations, or it hasn’t been held long enough yet.' },
  { status: 'red', name: 'Red', means: 'The submission was refused, the proof no longer verifies, or the fund missed two rounds.' },
  { status: 'none', name: 'Grey', means: 'Registered, with no proof yet.' },
] as const

const reducedMotion = () => window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false

export function Landing({ state }: { state: RegistryState | null }) {
  return (
    <div className="landing">
      <Hero />
      <Cases />
      <Anatomy />
      <Lights />
      <OnRegistry state={state} />
      <Treasurers />
      <footer className="l-foot">
        <span className="logo">
          <span className="mark" aria-hidden="true" />Z<span>-</span>Solvency
        </span>
        <span className="muted">Private proof of reserves on Zcash Ironwood. Open source; verify everything yourself.</span>
      </footer>
    </div>
  )
}

function Hero() {
  // One orchestrated moment: the ledger is shown, then redacted down to the line that gets published.
  const [redacted, setRedacted] = useState(reducedMotion)
  const [touched, setTouched] = useState(false)
  useEffect(() => {
    if (touched || redacted) return
    const t = setTimeout(() => setRedacted(true), 1600)
    return () => clearTimeout(t)
  }, [touched, redacted])

  const choose = (r: boolean) => {
    setTouched(true)
    setRedacted(r)
  }

  return (
    <section className="l-hero">
      <div className="l-hero-copy">
        <h1 className="l-display">Show the money, not the people.</h1>
        <p className="l-lede">
          Strike, legal-defence and mutual-aid funds prove they hold what they promised. Nobody learns who gave, or who
          got.
        </p>
        <div className="row">
          <a className="btn" href="#/funds">
            See the funds
          </a>
          <a className="btn quiet" href="#/list">
            List your fund
          </a>
        </div>
      </div>

      <figure className={`ledger ${redacted ? 'is-redacted' : ''}`}>
        <div className="ledger-head">
          <figcaption>
            <strong>Mill Workers’ Strike Fund</strong>
            <span className="muted">A fictional demo fund</span>
          </figcaption>
          <div className="toggle" role="group" aria-label="What the public sees">
            <button type="button" aria-pressed={!redacted} onClick={() => choose(false)}>
              Viewing key
            </button>
            <button type="button" aria-pressed={redacted} onClick={() => choose(true)}>
              Z-Solvency
            </button>
          </div>
        </div>

        <div className="ledger-body" aria-hidden={redacted}>
          <div className="ledger-balance">
            <span className="muted">Exact balance</span>
            <span className="r" style={{ '--i': 0 } as React.CSSProperties}>
              530 ZEC
            </span>
          </div>
          <table>
            <tbody>
              {LEDGER.map((l, i) => (
                <tr key={l.height} style={{ '--i': i + 1 } as React.CSSProperties}>
                  <td className="h">
                    <span className="r">#{l.height}</span>
                  </td>
                  <td>
                    <span className="r">{l.who}</span>
                    <span className="memo">
                      <span className="r">{l.memo}</span>
                    </span>
                  </td>
                  <td className="num">
                    <span className="r">{l.amount}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <div className="ledger-foot" aria-live="polite">
          {redacted ? (
            <>
              <p className="sr-only">The ledger is withheld. The public sees one line:</p>
              <p className="proof-line">≥ 480 ZEC · held since block 6 · checked at block 32 · signed by the fund</p>
            </>
          ) : (
            <p className="leak-line">A published viewing key shows every donor, payout and memo. Permanently.</p>
          )}
        </div>
      </figure>
    </section>
  )
}

function Cases() {
  return (
    <section className="l-section l-split">
      <div>
        <h2 className="l-h2">Funds that hold money for others are asked to show the books.</h2>
        <p className="muted">For a protest fund, the books are a list of donors and recipients.</p>
      </div>
      <ol className="cases">
        {CASES.map((c) => (
          <li key={c.name}>
            <span className="case-year">{c.year}</span>
            <div>
              <h3>{c.name}</h3>
              <p>{c.what}</p>
              <p className="case-lesson">{c.lesson}</p>
            </div>
          </li>
        ))}
      </ol>
    </section>
  )
}

function Anatomy() {
  const [active, setActive] = useState<number | null>(null)
  return (
    <section className="l-section">
      <h2 className="l-h2">One line, and everything it rules out</h2>
      <p className="muted l-narrow">
        Each round a fund publishes a single claim with two Halo 2 proofs behind it. Anyone can check it against roots
        rebuilt from public chain data.
      </p>
      <p className="anatomy-line" aria-hidden="true">
        {CLAUSES.map((c, i) => (
          <span key={c.text}>
            {i > 0 && <span className="sep">, </span>}
            <mark className={active === i ? 'on' : ''}>{c.text}</mark>
          </span>
        ))}
        .
      </p>
      <dl className="clauses">
        {CLAUSES.map((c, i) => (
          <div
            key={c.text}
            className={active === i ? 'on' : ''}
            onMouseEnter={() => setActive(i)}
            onMouseLeave={() => setActive(null)}
          >
            <dt>{c.text}</dt>
            <dd>
              {c.how}
              <span className="stops">Stops: {c.stops}</span>
            </dd>
          </div>
        ))}
      </dl>
      <aside className="limits">
        <h3>What it doesn’t prove</h3>
        <ul>
          <li>Where the coins came from.</li>
          <li>That the fund keeps them after the proof. Each round proves it again.</li>
          <li>That the published obligations are complete.</li>
          <li>That a friend didn’t lend the fund coins for months.</li>
        </ul>
      </aside>
    </section>
  )
}

function Lights() {
  return (
    <section className="l-section l-split">
      <div>
        <h2 className="l-h2">Read a fund at a glance</h2>
        <p className="muted">Every fund page carries one light. The first rule that matches sets it.</p>
      </div>
      <ul className="light-key">
        {LIGHTS.map((l) => (
          <li key={l.status}>
            <span className={`light ${l.status === 'none' ? '' : l.status}`} aria-hidden="true" />
            <div>
              <strong>{l.name}</strong>
              <p>{l.means}</p>
            </div>
          </li>
        ))}
      </ul>
    </section>
  )
}

function OnRegistry({ state }: { state: RegistryState | null }) {
  const funds = state?.funds.slice(0, 5) ?? []
  return (
    <section className="l-section">
      <div className="row between">
        <h2 className="l-h2">On this registry now</h2>
        {state && (
          <a className="more" href="#/funds">
            All {state.funds.length} fund{state.funds.length === 1 ? '' : 's'}
          </a>
        )}
      </div>
      {!state ? (
        <p className="muted">Connecting to the registry…</p>
      ) : funds.length === 0 ? (
        <p className="muted">
          No funds yet. <a href="#/list">Be the first to list one.</a>
        </p>
      ) : (
        <ul className="fund-rows">
          {funds.map((f) => (
            <li key={f.id}>
              <a className="fund-row" href={fundHref(f.id)}>
                <StatusDot status={f.status} />
                <span className="fund-row-text">
                  <span className="fund-name">{f.name}</span>
                  <span className="fund-sub">
                    {f.latest
                      ? `≥ ${zec(f.latest.threshold_zat)} ${f.unit} · held since block ${f.latest.since_height} · checked at block ${f.latest.at_height}`
                      : f.status_text}
                  </span>
                </span>
                <span className="chev" aria-hidden="true">
                  ›
                </span>
              </a>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

function Treasurers() {
  return (
    <section className="l-section l-dark">
      <div className="l-split">
        <div>
          <h2 className="l-h2">For treasurers</h2>
          <p>
            The fund console runs on your own laptop. The keys never leave it; only signed proofs reach the registry.
          </p>
          <pre>zsolv console</pre>
          <a className="btn invert" href="#/list">
            Read the full setup
          </a>
        </div>
        <ol className="t-steps">
          <li>
            <strong>Set up the wallet.</strong> Write down its 24 backup words. They restore the fund key too.
          </li>
          <li>
            <strong>Register the fund.</strong> Its name, what it’s for, its obligations, and how long it promises to
            hold the reserve.
          </li>
          <li>
            <strong>Lock the reserve.</strong> One note of exactly the obligations. Pay out from the rest.
          </li>
          <li>
            <strong>Prove and publish each round.</strong> It takes seconds. The page turns amber, then green once the
            holding promise passes.
          </li>
        </ol>
      </div>
    </section>
  )
}
