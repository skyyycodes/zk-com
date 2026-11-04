import { useEffect, useRef, useState } from 'react'
import { linkToken } from '../api'
import { linkHref } from '../hooks'
import type { RegistryState } from '../types'
import { Icon, prefersReducedMotion } from './kit'
import { jump } from './SiteNav'

export function Landing({ state, to }: { state: RegistryState | null; to: string | null }) {
  // Arriving from another page's nav link: go to the section it named.
  useEffect(() => {
    if (!to) return
    const t = setTimeout(() => document.getElementById(to)?.scrollIntoView({ block: 'start' }), 50)
    return () => clearTimeout(t)
  }, [to])

  return (
    <div className="landing">
      <Hero />
      <Proof />
      <How />
      <Privacy />
      <HaveLink />
      <Cta />
      <Footer state={state} />
    </div>
  )
}

/* ---------------- hero: the ledger, redacted to one line ---------------- */

const LEDGER = [
  { height: 21, who: 'to Orbit Family Office', amount: '−250', memo: 'withdrawal to cold wallet' },
  { height: 20, who: 'from Helios Capital', amount: '+300', memo: 'client deposit, acct 4471' },
  { height: 4, who: 'from Kestrel DAO', amount: '+900', memo: 'treasury custody' },
  { height: 3, who: 'from retail pool 12', amount: '+1,500', memo: 'omnibus deposit' },
  { height: 2, who: 'from Orbit Family Office', amount: '+1,800', memo: 'quarterly top-up' },
  { height: 1, who: 'from Helios Capital', amount: '+2,200', memo: 'client deposit, acct 4471' },
]

function Ledger() {
  const [redacted, setRedacted] = useState(prefersReducedMotion)
  const [touched, setTouched] = useState(false)
  // The page's one orchestrated moment: shortly after load, the ledger is redacted down to the line that gets published.
  useEffect(() => {
    if (touched || redacted) return
    const t = setTimeout(() => setRedacted(true), 1400)
    return () => clearTimeout(t)
  }, [touched, redacted])
  const choose = (r: boolean) => {
    setTouched(true)
    setRedacted(r)
  }

  return (
    <figure className={`ledger ${redacted ? 'is-redacted' : ''}`}>
      <div className="ledger-head">
        <figcaption>
          <span className="lg-name">Northwind Custody</span>
          <span className="lg-sub">example custody wallet</span>
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
          <span>Exact balance</span>
          <span className="r" style={{ '--i': 0 } as React.CSSProperties}>
            6,450 ZEC
          </span>
        </div>
        <table>
          <tbody>
            {LEDGER.map((l, i) => (
              <tr key={l.height} style={{ '--i': i + 1 } as React.CSSProperties}>
                <td className="h">#{l.height}</td>
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
            <p className="proof-line">
              <span className="live-dot" aria-hidden="true" />
              At least 5,000 ZEC, held since block 10, checked at block 36
            </p>
          </>
        ) : (
          <p className="leak-line">A shared viewing key shows every client, amount and memo, past and future.</p>
        )}
      </div>
    </figure>
  )
}

function Hero() {
  const [shown, setShown] = useState(false)
  useEffect(() => setShown(true), [])
  return (
    <section className={`l-hero ${shown ? 'in' : ''}`}>
      <div className="l-wrap hero-split">
        <div className="hero-text">
          <p className="hero-kicker rise">Private proof of reserves on Zcash</p>
          <h1 className="hero-title rise d1">Prove the reserve. Keep the books closed.</h1>
          <p className="hero-lede rise d2">
            Z-Solvency turns a shielded Zcash wallet into one public statement: at least T ZEC, held since block X,
            controlled by the holder, and counted by no one else. Anyone can check it against the chain. Nothing else
            leaves the wallet.
          </p>
          <div className="hero-ctas rise d3">
            <a className="btn lg" href="#/create">
              Create your link
            </a>
            <a className="btn quiet lg" href="#/verify">
              Verify a link
            </a>
          </div>
        </div>
        <div className="hero-ledger rise d2">
          <Ledger />
        </div>
      </div>
    </section>
  )
}

/* ---------------- what the statement guarantees ---------------- */

/** Diagrams hold still until pointed at: motion answers the reader, it doesn't compete for attention. */
function Visual({ children }: { children: React.ReactNode }) {
  const ref = useRef<SVGSVGElement>(null)
  useEffect(() => {
    ref.current?.pauseAnimations()
  }, [])
  const play = (on: boolean) => {
    if (prefersReducedMotion()) return
    if (on) ref.current?.unpauseAnimations()
    else ref.current?.pauseAnimations()
  }
  return (
    <span className="f-art" onMouseEnter={() => play(true)} onMouseLeave={() => play(false)}>
      <svg ref={ref} viewBox="0 0 200 160" className="f-visual" aria-hidden="true">
        {children}
      </svg>
    </span>
  )
}

function ThresholdVisual() {
  const heights = [70, 95, 60, 110, 85]
  return (
    <Visual>
      <line x1="20" y1="140" x2="180" y2="140" stroke="currentColor" strokeWidth="1.5" />
      {heights.map((h, i) => (
        <rect key={i} x={34 + i * 28} width="16" y={140 - h} height={h} fill="currentColor" opacity="0.18">
          <animate attributeName="opacity" values="0.18;0.6;0.18" dur="2.4s" begin={`${i * 0.2}s`} repeatCount="indefinite" />
        </rect>
      ))}
      <line x1="20" y1="60" x2="180" y2="60" stroke="currentColor" strokeWidth="1.5" strokeDasharray="5 5">
        <animate attributeName="stroke-dashoffset" values="0;-10" dur="0.8s" repeatCount="indefinite" />
      </line>
      <text x="184" y="64" fontSize="13" fontFamily="monospace" fill="currentColor">T</text>
      <rect x="20" y="16" width="160" height="34" style={{ fill: 'var(--paper)' }} />
      <text x="100" y="38" textAnchor="middle" fontSize="12" fontFamily="monospace" fill="currentColor" opacity="0.6">balance hidden</text>
    </Visual>
  )
}

function HeldVisual() {
  return (
    <Visual>
      {[['X', 30], ['H', 120]].map(([l, x]) => (
        <g key={l}>
          <rect x={x} y="50" width="50" height="60" fill="none" stroke="currentColor" strokeWidth="1.5" />
          <text x={Number(x) + 25} y="86" textAnchor="middle" fontSize="20" fontFamily="monospace" fill="currentColor">{l}</text>
        </g>
      ))}
      <line x1="80" y1="80" x2="120" y2="80" stroke="currentColor" strokeWidth="1.5" strokeDasharray="4 4">
        <animate attributeName="stroke-dashoffset" values="0;-8" dur="0.5s" repeatCount="indefinite" />
      </line>
      <circle cx="80" cy="80" r="4" fill="currentColor">
        <animateMotion dur="1.5s" repeatCount="indefinite" path="M 0 0 L 40 0" />
      </circle>
      <text x="100" y="140" textAnchor="middle" fontSize="11" fontFamily="monospace" fill="currentColor" opacity="0.6">same notes</text>
    </Visual>
  )
}

function SignedVisual() {
  return (
    <Visual>
      <path d="M 100 20 L 150 40 L 150 90 Q 150 130 100 145 Q 50 130 50 90 L 50 40 Z" fill="none" stroke="currentColor" strokeWidth="1.5" />
      <path d="M 100 35 L 135 50 L 135 85 Q 135 115 100 128 Q 65 115 65 85 L 65 50 Z" fill="currentColor" opacity="0.1">
        <animate attributeName="opacity" values="0.1;0.25;0.1" dur="2s" repeatCount="indefinite" />
      </path>
      <rect x="85" y="70" width="30" height="25" rx="2" fill="currentColor" />
      <path d="M 90 70 L 90 60 Q 90 50 100 50 Q 110 50 110 60 L 110 70" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
      <circle cx="100" cy="80" r="4" style={{ fill: 'var(--paper)' }} />
      <rect x="98" y="82" width="4" height="8" style={{ fill: 'var(--paper)' }} />
    </Visual>
  )
}

function RegistryVisual() {
  return (
    <Visual>
      <circle cx="100" cy="80" r="12" fill="currentColor">
        <animate attributeName="r" values="12;14;12" dur="2s" repeatCount="indefinite" />
      </circle>
      {[0, 1, 2, 3, 4, 5].map((i) => {
        const a = (i * 60 * Math.PI) / 180
        const x = 100 + Math.cos(a) * 50
        const y = 80 + Math.sin(a) * 50
        const refused = i === 4
        return (
          <g key={i}>
            <line x1="100" y1="80" x2={x} y2={y} stroke="currentColor" strokeWidth="1" opacity="0.35" strokeDasharray={refused ? '3 3' : undefined}>
              {!refused && <animate attributeName="opacity" values="0.35;0.85;0.35" dur="2s" begin={`${i * 0.3}s`} repeatCount="indefinite" />}
            </line>
            <circle cx={x} cy={y} r="6" fill="none" stroke="currentColor" strokeWidth="1.5" opacity={refused ? 0.45 : 1} />
            {refused && <path d={`M ${x - 4} ${y - 4} L ${x + 4} ${y + 4} M ${x + 4} ${y - 4} L ${x - 4} ${y + 4}`} stroke="currentColor" strokeWidth="1.5" />}
          </g>
        )
      })}
    </Visual>
  )
}

const PROOF = [
  {
    title: 'At least T, never the balance',
    text: 'A zero-knowledge proof shows the notes add up to at least the threshold. The actual total, the notes and the addresses stay private. Over-claiming is impossible in the proof itself.',
    visual: <ThresholdVisual />,
  },
  {
    title: 'Held since block X',
    text: 'Two proofs over the same notes, one at X and one at H. Their fingerprints must match, so coins borrowed for the day of the check cannot pass.',
    visual: <HeldVisual />,
  },
  {
    title: 'Controlled by the holder',
    text: 'The notes’ own spending key signs each statement, and so does the key bound to the holder when it registered. A viewing key, or a copy of someone else’s proof, is not enough.',
    visual: <SignedVisual />,
  },
  {
    title: 'Counted once',
    text: 'Each note leaves a fingerprint scoped to the registry, never its real nullifier. A second holder claiming the same coins is refused, and nobody learns which coins they were.',
    visual: <RegistryVisual />,
  },
]

function Proof() {
  return (
    <section id="proof" className="l-section">
      <div className="l-wrap">
        <h2 className="l-h2">Four things the statement guarantees</h2>
        <div className="g-grid">
          {PROOF.map((p) => (
            <div key={p.title} className="g-cell">
              <div>
                <h3>{p.title}</h3>
                <p>{p.text}</p>
              </div>
              {p.visual}
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

/* ---------------- how it works (ink) ---------------- */

const STEPS = [
  {
    title: 'Register the holder',
    text: 'In the console on your own machine: name the holder, say what the reserve backs, list the obligations and how long the reserve will be held.',
    file: 'register',
    code: `$ zsolv fund --wallet northwind --id northwind \\
    --name "Northwind Custody" \\
    --obligation "Client balances=5000" \\
    --min-held-blocks 20

# the holder key is bound to "northwind"
# the registry returns its share link`,
  },
  {
    title: 'Lock the reserve',
    text: 'One note of exactly the obligations, sent to the holder’s own address. Day-to-day payments come from the rest.',
    file: 'reserve',
    code: `$ zsolv reserve --wallet northwind --amount 5000

# one note of exactly T
# "held since" starts at this block`,
  },
  {
    title: 'Prove and publish',
    text: 'Each round, on your own machine. The registry verifies the statement; the link turns amber, then green once the reserve has been held as promised.',
    file: 'prove',
    code: `$ zsolv prove --wallet northwind --fund-id northwind \\
    --threshold 5000 --since 10 --out northwind.json
$ zsolv submit northwind.json

# 2 Halo 2 proofs, signed by the holder`,
  },
  {
    title: 'Anyone with the link verifies',
    text: 'Auditors, clients and counterparties open the link, or download the statement and check it against the chain themselves.',
    file: 'verify',
    code: `$ curl -s $REGISTRY/api/links/$LINK/attestation \\
    > northwind.json
$ zsolv verify northwind.json

# valid: at least 5000 ZEC since block 10`,
  },
]

function How() {
  const [active, setActive] = useState(0)
  const step = STEPS[active]
  return (
    <section id="how" className="l-section l-dark">
      <div className="l-wrap">
        <h2 className="l-h2">Four steps each round. Keys never leave your machine.</h2>
        <div className="how-grid">
          <ol className="how-steps">
            {STEPS.map((s, i) => (
              <li key={s.title}>
                <button type="button" className={`how-step ${i === active ? 'on' : ''}`} onClick={() => setActive(i)} aria-pressed={i === active}>
                  <span className="how-n">{i + 1}</span>
                  <span className="how-body">
                    <span className="how-title">{s.title}</span>
                    <span className="how-text">{s.text}</span>
                  </span>
                </button>
              </li>
            ))}
          </ol>
          <div className="code-win">
            <div className="code-head">
              <span className="code-file">{step.file}.sh</span>
              <span className="code-file">
                step {active + 1} of {STEPS.length}
              </span>
            </div>
            <pre className="code-body" key={active}>
              {step.code.split('\n').map((line, li) => (
                <div key={li} className="code-line" style={{ animationDelay: `${li * 60}ms` }}>
                  <span className="ln">{li + 1}</span>
                  <span className={line.startsWith('#') ? 'cm' : undefined}>{line || ' '}</span>
                </div>
              ))}
            </pre>
            <div className="code-foot">
              Or do it all from <code>zsolv console</code>, in your browser
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

/* ---------------- privacy: what stays private, what it doesn't prove ---------------- */

const HIDDEN = ['Exact balance', 'Addresses', 'Counterparties', 'Amounts', 'Memos', 'Transaction history', 'Future activity']

const LIMITS = [
  'Where the coins came from.',
  'That nothing else is owed beyond the obligations you publish.',
  'That the coins stay after the statement. Each round proves it again.',
]

function Privacy() {
  return (
    <section id="privacy" className="l-section">
      <div className="l-wrap privacy-grid">
        <div>
          <h2 className="l-h2">A viewing key opens the whole ledger. This shows one line.</h2>
          <p className="l-lede">
            Sharing a viewing key proves a balance by exposing every deposit, withdrawal, counterparty and memo, past and
            future, with no way to take it back. Z-Solvency proves the reserve and publishes nothing else. A new statement
            each round replaces the last.
          </p>
          <ul className="chips" aria-label="Never published">
            {HIDDEN.map((h) => (
              <li key={h}>
                <s>{h}</s>
              </li>
            ))}
          </ul>
        </div>
        <div className="limits">
          <h3>What it does not prove</h3>
          <ul>
            {LIMITS.map((l) => (
              <li key={l}>{l}</li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  )
}

/* ---------------- have a link? ---------------- */

function HaveLink() {
  const [value, setValue] = useState('')
  const [error, setError] = useState<string | null>(null)
  return (
    <section id="verify" className="l-section bordered-top">
      <div className="l-wrap have-grid">
        <div>
          <h2 className="l-h2">Statements aren’t listed. Holders share a link.</h2>
          <p className="l-lede">
            Nobody can browse the holders on this registry. Each one gives its link to the auditors, clients and
            counterparties it answers to, and anyone with the link can check the proof.
          </p>
        </div>
        <form
          className="have-form"
          noValidate
          onSubmit={(e) => {
            e.preventDefault()
            const t = linkToken(value)
            if (t) window.location.hash = linkHref(t)
            else setError('That isn’t a Z-Solvency link. It looks like …/#/v/ followed by 32 letters and digits.')
          }}
        >
          <label htmlFor="have-link" className="have-label">
            Paste a link
          </label>
          <div className="have-row">
            <input
              id="have-link"
              type="url"
              inputMode="url"
              autoComplete="off"
              spellCheck={false}
              placeholder={`${window.location.host}/#/v/…`}
              value={value}
              onChange={(e) => {
                setValue(e.target.value)
                setError(null)
              }}
              aria-invalid={error ? true : undefined}
              aria-describedby={error ? 'have-error' : undefined}
            />
            <button type="submit">Verify</button>
          </div>
          {error && (
            <p className="field-error" id="have-error">
              {error}
            </p>
          )}
        </form>
      </div>
    </section>
  )
}

/* ---------------- call to action ---------------- */

function Cta() {
  return (
    <section className="l-section l-cta">
      <div className="l-wrap cta-inner">
        <h2 className="l-h2">Publish your first statement.</h2>
        <div>
          <p className="l-lede">
            Free and open source. It runs on your own machine, and the registry only ever sees the statement. Set up a
            reserve on the local devnet in about ten minutes.
          </p>
          <div className="hero-ctas">
            <a className="btn lg" href="#/create">
              Create your link <Icon name="arrowRight" />
            </a>
            <a className="btn quiet lg" href="#/verify">
              Verify a link
            </a>
          </div>
        </div>
      </div>
    </section>
  )
}

/* ---------------- footer ---------------- */

const FOOT = {
  Statements: [
    { name: 'Create your link', href: '#/create' },
    { name: 'Verify a link', href: '#/verify' },
  ],
  Learn: [
    { name: 'What it guarantees', href: '#/?to=proof' },
    { name: 'How it works', href: '#/?to=how' },
    { name: 'Why not a viewing key', href: '#/?to=privacy' },
  ],
}

function Footer({ state }: { state: RegistryState | null }) {
  return (
    <footer className="l-foot">
      <div className="l-wrap">
        <div className="foot-grid">
          <div className="foot-brand">
            <a href="#/" className="logo">
              <span className="word">Z-Solvency</span>
            </a>
            <p>Private proof of reserves on Zcash. Prove the reserve, keep the books closed.</p>
          </div>
          {Object.entries(FOOT).map(([title, links]) => (
            <nav key={title} aria-label={title}>
              <h3>{title}</h3>
              <ul>
                {links.map((l) => (
                  <li key={l.name}>
                    <a href={l.href} onClick={(e) => jump(e, l.href)}>
                      {l.name}
                    </a>
                  </li>
                ))}
              </ul>
            </nav>
          ))}
          <div>
            <h3>Verify</h3>
            <p className="foot-small">
              Every statement page has a Verify button, and <code>zsolv verify</code> checks any statement on your own
              machine.
            </p>
          </div>
        </div>
        <div className="foot-bar">
          <p>Open source. No analytics, no third-party scripts.</p>
          <span className="live">
            <span className="live-dot" />
            {state ? `Registry online, ${state.network}, tip ${state.tip}` : 'Connecting to the registry'}
          </span>
        </div>
      </div>
    </footer>
  )
}
