import { useEffect, useRef, useState } from 'react'
import { zec } from '../format'
import { fundHref } from '../hooks'
import type { RegistryState } from '../types'
import { StatusDot } from '../components/StatusLight'
import { AsciiCanvas, Icon, Label, prefersReducedMotion, useInView } from './kit'
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
      <Live state={state} />
      <Privacy />
      <Cases />
      <Funds state={state} />
      <Cta />
      <Footer state={state} />
    </div>
  )
}

/* ---------------- hero ---------------- */

const WORDS = ['people', 'donors', 'members', 'memos']

const MARQUEE = [
  { value: '0', label: 'donors revealed', tag: 'Zero knowledge' },
  { value: '2', label: 'Halo 2 proofs a round', tag: 'No trusted setup' },
  { value: '22 KiB', label: 'per attestation', tag: 'Anyone can verify' },
  { value: '5', label: 'nullifiers per proof', tag: 'One note, one fund' },
  { value: '≈5 s', label: 'to prove a round', tag: 'On a laptop' },
]

function Hero() {
  const [shown, setShown] = useState(false)
  const [word, setWord] = useState(0)
  useEffect(() => setShown(true), [])
  useEffect(() => {
    if (prefersReducedMotion()) return
    const t = setInterval(() => setWord((w) => (w + 1) % WORDS.length), 2500)
    return () => clearInterval(t)
  }, [])

  return (
    <section className={`l-hero ${shown ? 'in' : ''}`}>
      <div className="hero-sphere">
        <AsciiCanvas shape="sphere" />
      </div>
      <div className="hero-grid" aria-hidden="true">
        {Array.from({ length: 7 }, (_, i) => (
          <span key={`h${i}`} className="gh" style={{ top: `${12.5 * (i + 1)}%` }} />
        ))}
        {Array.from({ length: 11 }, (_, i) => (
          <span key={`v${i}`} className="gv" style={{ left: `${8.33 * (i + 1)}%` }} />
        ))}
      </div>

      <div className="l-wrap hero-body">
        <div className="rise d0">
          <Label>Private proof of reserves on Zcash</Label>
        </div>
        <h1 className="hero-title rise d1">
          <span className="line">Show the money,</span>
          <span className="line">
            not the{' '}
            <span className="swap">
              <span key={word} className="swap-word" aria-hidden="true">
                {WORDS[word].split('').map((c, i) => (
                  <span key={i} className="char-in" style={{ animationDelay: `${i * 50}ms` }}>
                    {c}
                  </span>
                ))}
              </span>
              <span className="sr-only">people</span>
              <span className="swap-bar" aria-hidden="true" />
            </span>
          </span>
        </h1>
        <div className="hero-foot">
          <p className="hero-lede rise d2">
            Strike, legal-defence and mutual-aid funds prove they hold what they promised. Nobody learns who gave, or who
            got.
          </p>
          <div className="hero-ctas rise d3">
            <a className="btn lg" href="#/funds">
              See the funds <Icon name="arrowRight" />
            </a>
            <a className="btn quiet lg" href="#/list">
              List your fund
            </a>
          </div>
        </div>
      </div>

      <div className="hero-marquee rise d4" aria-label="Key figures">
        <div className="marquee">
          {[0, 1].map((copy) => (
            <div key={copy} className="marquee-set" aria-hidden={copy === 1}>
              {MARQUEE.map((m) => (
                <div key={m.label} className="mq-item">
                  <span className="mq-value">{m.value}</span>
                  <span className="mq-label">
                    {m.label}
                    <span className="mq-tag">{m.tag}</span>
                  </span>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

/* ---------------- what the proof says ---------------- */

/** SMIL keeps running regardless of the media query, so pause it by hand. */
function Visual({ children }: { children: React.ReactNode }) {
  const ref = useRef<SVGSVGElement>(null)
  useEffect(() => {
    if (prefersReducedMotion()) ref.current?.pauseAnimations()
  }, [])
  return (
    <svg ref={ref} viewBox="0 0 200 160" className="f-visual" aria-hidden="true">
      {children}
    </svg>
  )
}

function ThresholdVisual() {
  const heights = [70, 95, 60, 110, 85]
  return (
    <Visual>
      <line x1="20" y1="140" x2="180" y2="140" stroke="currentColor" strokeWidth="2" />
      {heights.map((h, i) => (
        <rect key={i} x={34 + i * 28} width="16" y={140 - h} height={h} fill="currentColor" opacity="0.15">
          <animate attributeName="opacity" values="0.15;0.6;0.15" dur="2.4s" begin={`${i * 0.2}s`} repeatCount="indefinite" />
        </rect>
      ))}
      <line x1="20" y1="60" x2="180" y2="60" stroke="currentColor" strokeWidth="2" strokeDasharray="5 5">
        <animate attributeName="stroke-dashoffset" values="0;-10" dur="0.8s" repeatCount="indefinite" />
      </line>
      <text x="184" y="64" fontSize="13" fontFamily="monospace" fill="currentColor">T</text>
      <rect x="20" y="16" width="160" height="34" fill="var(--paper)" />
      <text x="100" y="38" textAnchor="middle" fontSize="12" fontFamily="monospace" fill="currentColor" opacity="0.6">balance hidden</text>
    </Visual>
  )
}

function HeldVisual() {
  return (
    <Visual>
      {[['X', 30], ['H', 120]].map(([l, x]) => (
        <g key={l}>
          <rect x={x} y="50" width="50" height="60" rx="4" fill="none" stroke="currentColor" strokeWidth="2" />
          <text x={Number(x) + 25} y="86" textAnchor="middle" fontSize="20" fontFamily="monospace" fill="currentColor">{l}</text>
        </g>
      ))}
      <line x1="80" y1="80" x2="120" y2="80" stroke="currentColor" strokeWidth="2" strokeDasharray="4 4">
        <animate attributeName="stroke-dashoffset" values="0;-8" dur="0.5s" repeatCount="indefinite" />
      </line>
      <circle r="4" fill="currentColor">
        <animateMotion dur="1.5s" repeatCount="indefinite" path="M 80 80 L 120 80" />
      </circle>
      <text x="100" y="140" textAnchor="middle" fontSize="11" fontFamily="monospace" fill="currentColor" opacity="0.6">same notes</text>
    </Visual>
  )
}

function SignedVisual() {
  return (
    <Visual>
      <path d="M 100 20 L 150 40 L 150 90 Q 150 130 100 145 Q 50 130 50 90 L 50 40 Z" fill="none" stroke="currentColor" strokeWidth="2" />
      <path d="M 100 35 L 135 50 L 135 85 Q 135 115 100 128 Q 65 115 65 85 L 65 50 Z" fill="currentColor" opacity="0.1">
        <animate attributeName="opacity" values="0.1;0.2;0.1" dur="2s" repeatCount="indefinite" />
      </path>
      <rect x="85" y="70" width="30" height="25" rx="3" fill="currentColor" />
      <path d="M 90 70 L 90 60 Q 90 50 100 50 Q 110 50 110 60 L 110 70" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
      <circle cx="100" cy="80" r="4" style={{ fill: 'var(--paper)' }} />
      <rect x="98" y="82" width="4" height="8" style={{ fill: 'var(--paper)' }} />
      <line x1="60" y1="60" x2="140" y2="60" stroke="currentColor" strokeWidth="1" opacity="0">
        <animate attributeName="y1" values="40;120;40" dur="3s" repeatCount="indefinite" />
        <animate attributeName="y2" values="40;120;40" dur="3s" repeatCount="indefinite" />
        <animate attributeName="opacity" values="0;0.5;0" dur="3s" repeatCount="indefinite" />
      </line>
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
            <line x1="100" y1="80" x2={x} y2={y} stroke="currentColor" strokeWidth="1" opacity="0.3" strokeDasharray={refused ? '3 3' : undefined}>
              {!refused && <animate attributeName="opacity" values="0.3;0.8;0.3" dur="2s" begin={`${i * 0.3}s`} repeatCount="indefinite" />}
            </line>
            <circle cx={x} cy={y} r="6" fill="none" stroke="currentColor" strokeWidth="2" opacity={refused ? 0.4 : 1} />
            {refused && <path d={`M ${x - 4} ${y - 4} L ${x + 4} ${y + 4} M ${x + 4} ${y - 4} L ${x - 4} ${y + 4}`} stroke="currentColor" strokeWidth="2" />}
          </g>
        )
      })}
      <circle cx="100" cy="80" r="30" fill="none" stroke="currentColor" strokeWidth="1" opacity="0">
        <animate attributeName="r" values="20;60" dur="2s" repeatCount="indefinite" />
        <animate attributeName="opacity" values="0.5;0" dur="2s" repeatCount="indefinite" />
      </circle>
    </Visual>
  )
}

const PROOF = [
  { n: '01', title: 'At least T, never the balance', text: 'The circuit commits to the notes’ total, and the fund opens it only as far as the threshold. Over-claiming is impossible in the proof itself.', visual: <ThresholdVisual /> },
  { n: '02', title: 'Held since block X', text: 'Two proofs over the same notes, at X and at H. Their nullifiers must match, so coins borrowed for audit day can’t pass.', visual: <HeldVisual /> },
  { n: '03', title: 'Signed by the fund', text: 'The notes’ own spend key signs the round, and so does the fund key bound when the fund first registered. Nobody can publish in its name.', visual: <SignedVisual /> },
  { n: '04', title: 'Counted by no other fund', text: 'Each note shows a nullifier scoped to this registry, never its real one. A second fund claiming the same coins is refused.', visual: <RegistryVisual /> },
]

function ProofRow({ p, i }: { p: (typeof PROOF)[number]; i: number }) {
  const [ref, seen] = useInView<HTMLDivElement>(0.2)
  return (
    <div ref={ref} className={`f-row reveal ${seen ? 'in' : ''}`} style={{ transitionDelay: `${i * 100}ms` }}>
      <span className="f-num">{p.n}</span>
      <div className="f-body">
        <div>
          <h3>{p.title}</h3>
          <p>{p.text}</p>
        </div>
        <div className="f-art">{p.visual}</div>
      </div>
    </div>
  )
}

function Proof() {
  const [ref, seen] = useInView<HTMLDivElement>(0.1)
  return (
    <section id="proof" className="l-section">
      <div ref={ref} className="l-wrap">
        <header className="l-head">
          <Label>What the proof says</Label>
          <h2 className={`l-h2 reveal ${seen ? 'in' : ''}`}>
            Everything a donor needs.
            <br />
            <span className="dim">Nothing they don’t.</span>
          </h2>
        </header>
        {PROOF.map((p, i) => (
          <ProofRow key={p.n} p={p} i={i} />
        ))}
      </div>
    </section>
  )
}

/* ---------------- how it works (dark) ---------------- */

const STEPS = [
  {
    n: 'I',
    title: 'Register the fund',
    text: 'Name it, say what it’s for, list its obligations and how long it promises to hold the reserve. The page goes live, grey.',
    file: 'register',
    code: `$ zsolv fund --wallet mill --id mill \\
    --name "Mill Workers' Strike Fund" \\
    --obligation "Strike pay=480" \\
    --min-held-blocks 20

# the fund key is bound to "mill"`,
  },
  {
    n: 'II',
    title: 'Lock the reserve',
    text: 'One note of exactly the obligations, sent to yourself. Pay strike pay and costs from the rest.',
    file: 'reserve',
    code: `$ zsolv reserve --wallet mill --amount 480

# one note of exactly T
# "held since" starts at this block`,
  },
  {
    n: 'III',
    title: 'Prove and publish',
    text: 'Every round, on your own laptop. The registry verifies it and the page turns amber, then green.',
    file: 'prove',
    code: `$ zsolv prove --wallet mill --fund-id mill \\
    --threshold 480 --since 6 --out mill.json
$ zsolv submit mill.json

# 2 Halo 2 proofs, signed by the fund`,
  },
  {
    n: 'IV',
    title: 'Anyone verifies',
    text: 'Donors, members or journalists rebuild the roots from public chain data and check it themselves.',
    file: 'verify',
    code: `$ curl -s $REGISTRY/api/funds/mill/attestation \\
    > mill.json
$ zsolv verify mill.json

# valid: at least 480 ZEC since block 6`,
  },
]

function How() {
  const [active, setActive] = useState(0)
  const [ref, seen] = useInView<HTMLElement>(0.1)
  const still = prefersReducedMotion()
  useEffect(() => {
    if (still) return
    const t = setInterval(() => setActive((a) => (a + 1) % STEPS.length), 6000)
    return () => clearInterval(t)
  }, [active, still])

  const step = STEPS[active]
  return (
    <section id="how" ref={ref} className="l-section l-dark">
      <div className="diag" aria-hidden="true" />
      <div className="l-wrap">
        <header className="l-head">
          <Label inverted>For treasurers</Label>
          <h2 className={`l-h2 reveal ${seen ? 'in' : ''}`}>
            Four steps.
            <br />
            <span className="dim">Keys never leave your laptop.</span>
          </h2>
        </header>
        <div className="how-grid">
          <div className="how-steps">
            {STEPS.map((s, i) => (
              <button key={s.n} type="button" className={`how-step ${i === active ? 'on' : ''}`} onClick={() => setActive(i)} aria-pressed={i === active}>
                <span className="how-n">{s.n}</span>
                <span className="how-body">
                  <span className="how-title">{s.title}</span>
                  <span className="how-text">{s.text}</span>
                  {i === active && !still && (
                    <span className="how-progress" aria-hidden="true">
                      <span />
                    </span>
                  )}
                </span>
              </button>
            ))}
          </div>
          <div className="code-win">
            <div className="code-head">
              <span className="dots" aria-hidden="true">
                <i />
                <i />
                <i />
              </span>
              <span className="code-file">{step.file}.sh</span>
            </div>
            <pre className="code-body" key={active}>
              {step.code.split('\n').map((line, li) => (
                <div key={li} className="code-line" style={{ animationDelay: `${li * 80}ms` }}>
                  <span className="ln">{li + 1}</span>
                  <span className={line.startsWith('#') ? 'cm' : undefined}>{line || ' '}</span>
                </div>
              ))}
            </pre>
            <div className="code-foot">
              <span className="live-dot" />
              Or do it all from <code>zsolv console</code>, in your browser
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

/* ---------------- live registry metrics ---------------- */

function Counter({ value }: { value: number | null }) {
  const [n, setN] = useState(0)
  const [ref, seen] = useInView<HTMLDivElement>(0.5)
  useEffect(() => {
    if (!seen || value == null) return
    if (prefersReducedMotion()) return setN(value)
    const start = performance.now()
    let raf = 0
    const tick = (now: number) => {
      const p = Math.min((now - start) / 1600, 1)
      setN(Math.floor((1 - Math.pow(1 - p, 3)) * value))
      if (p < 1) raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [seen, value])
  return (
    <div ref={ref} className="m-value">
      {value == null ? '–' : n.toLocaleString('en-US')}
    </div>
  )
}

function Live({ state }: { state: RegistryState | null }) {
  const [time, setTime] = useState(() => new Date())
  const [ref, seen] = useInView<HTMLElement>(0.1)
  useEffect(() => {
    const t = setInterval(() => setTime(new Date()), 1000)
    return () => clearInterval(t)
  }, [])
  const funds = state?.funds ?? []
  const metrics = [
    { value: state ? funds.length : null, label: 'Funds on this registry' },
    { value: state ? funds.filter((f) => f.status === 'green').length : null, label: 'Funds green right now' },
    { value: state ? funds.reduce((n, f) => n + f.rounds, 0) : null, label: 'Rounds published' },
    { value: state ? state.tip : null, label: `Chain tip, ${state?.network ?? 'connecting'}` },
  ]
  return (
    <section id="registry" ref={ref} className="l-section bordered">
      <div className="l-wrap">
        <header className="l-head split">
          <div>
            <Label>Live registry</Label>
            <h2 className={`l-h2 reveal ${seen ? 'in' : ''}`}>
              Proofs you can
              <br />
              check right now.
            </h2>
          </div>
          <div className="live-clock">
            <span className="live">
              <span className="live-dot" /> Live
            </span>
            <span className="sep">|</span>
            <span>{time.toLocaleTimeString()}</span>
          </div>
        </header>
        <div className="metrics">
          {metrics.map((m, i) => (
            <div key={m.label} className={`metric reveal ${seen ? 'in' : ''}`} style={{ transitionDelay: `${i * 100}ms` }}>
              <Counter value={m.value} />
              <div className="m-label">{m.label}</div>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

/* ---------------- privacy: viewing key vs Z-Solvency ---------------- */

const LEDGER = [
  { height: 22, who: 'to members', amount: '−30', memo: 'strike pay wk2: S. Haddad, P. Ruiz, +28' },
  { height: 21, who: 'from anonymous', amount: '+25', memo: 'from a manager who agrees' },
  { height: 14, who: 'to members', amount: '−30', memo: 'strike pay wk1: M. Okafor, J. Lindqvist, +28' },
  { height: 13, who: 'from Bakery co-op', amount: '+40', memo: 'bread money' },
  { height: 4, who: "from a nurse at St. Anne's", amount: '+60', memo: 'keep going' },
]

const HIDDEN = ['Exact balance', 'Donor names', 'Recipients', 'Memos', 'Payment history']

function Ledger() {
  const [ref, seen] = useInView<HTMLElement>(0.4)
  const [redacted, setRedacted] = useState(prefersReducedMotion)
  const [touched, setTouched] = useState(false)
  // One orchestrated moment: once in view, the ledger is redacted down to the line that gets published.
  useEffect(() => {
    if (!seen || touched || redacted) return
    const t = setTimeout(() => setRedacted(true), 900)
    return () => clearTimeout(t)
  }, [seen, touched, redacted])
  const choose = (r: boolean) => {
    setTouched(true)
    setRedacted(r)
  }

  return (
    <figure ref={ref} className={`ledger ${redacted ? 'is-redacted' : ''}`}>
      <div className="ledger-head">
        <figcaption>
          <span className="lg-name">Mill Workers’ Strike Fund</span>
          <span className="lg-sub">fictional demo fund</span>
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
            <p className="proof-line">
              <span className="live-dot" aria-hidden="true" />≥ 480 ZEC · held since block 6 · checked at block 32 · signed by the fund
            </p>
          </>
        ) : (
          <p className="leak-line">A published viewing key shows every donor, payout and memo. Permanently.</p>
        )}
      </div>
    </figure>
  )
}

function Privacy() {
  const [ref, seen] = useInView<HTMLElement>(0.1)
  return (
    <section id="privacy" ref={ref} className="l-section tinted">
      <div className="l-wrap privacy-grid">
        <div className={`reveal ${seen ? 'in' : ''}`}>
          <Label>Why not a viewing key</Label>
          <h2 className="l-h2">
            Trust without
            <br />
            the ledger.
          </h2>
          <p className="l-lede">
            A published viewing key would prove the balance, but it is permanent and exact, and it shows every donation,
            memo and payout. Z-Solvency publishes one line instead, and a new one each round.
          </p>
          <ul className="chips" aria-label="Never published">
            {HIDDEN.map((h, i) => (
              <li key={h} className={`reveal ${seen ? 'in' : ''}`} style={{ transitionDelay: `${i * 50 + 200}ms` }}>
                <s>{h}</s>
              </li>
            ))}
          </ul>
          <p className="fineprint">
            It doesn’t prove where the coins came from, that the fund keeps them after the proof, or that the obligations
            are complete. Each round proves it again.
          </p>
        </div>
        <div className={`reveal from-right ${seen ? 'in' : ''}`}>
          <Ledger />
        </div>
      </div>
    </section>
  )
}

/* ---------------- why it matters: real cases ---------------- */

const CASES = [
  {
    text: 'Paid medical and legal fees for 2019 protesters. Its trustees were arrested in 2022, and six people were convicted of failing to register the fund.',
    name: '612 Humanitarian Relief Fund',
    where: 'Hong Kong, 2021',
    lesson: 'Legal-defence funds become targets.',
  },
  {
    text: 'A breach of the donation platform exposed 92,845 donors to a protest convoy by name.',
    name: 'Freedom Convoy donors',
    where: 'Canada, 2022',
    lesson: 'Donor lists leak.',
  },
  {
    text: 'During #EndSARS its bank account and payment link were cut. A year later, critics demanded it account for the money.',
    name: 'Feminist Coalition',
    where: 'Nigeria, 2020',
    lesson: 'A fund gets cut off, then doubted.',
  },
]

const KINDS = ['Strike funds', 'Legal-defence funds', 'Mutual-aid funds', 'Bail funds', 'Solidarity funds', 'Relief funds', 'Community kitchens', 'Union hardship funds']

function Cases() {
  const [i, setI] = useState(0)
  const [fading, setFading] = useState(false)
  const go = (n: number) => {
    setFading(true)
    setTimeout(() => {
      setI(n)
      setFading(false)
    }, 300)
  }
  useEffect(() => {
    if (prefersReducedMotion()) return
    const t = setInterval(() => go((i + 1) % CASES.length), 7000)
    return () => clearInterval(t)
  }, [i])
  const c = CASES[i]

  return (
    <section className="l-section cases">
      <div className="l-wrap">
        <div className="cases-label">
          <span>Why it matters</span>
          <span className="rule" />
          <span>
            {String(i + 1).padStart(2, '0')} / {String(CASES.length).padStart(2, '0')}
          </span>
        </div>
        <div className="cases-grid" aria-live="polite">
          <div>
            <blockquote className={`case-quote ${fading ? 'out' : ''}`}>
              <p>{c.text}</p>
            </blockquote>
            <div className={`case-who ${fading ? 'out' : ''}`}>
              <span className="case-mono" aria-hidden="true">{c.where.charAt(0)}</span>
              <span>
                <span className="case-name">{c.name}</span>
                <span className="case-where">{c.where}</span>
              </span>
            </div>
          </div>
          <div className="case-side">
            <div className={`case-lesson ${fading ? 'out' : ''}`}>
              <span className="case-k">What it shows</span>
              <p>{c.lesson}</p>
            </div>
            <div className="case-dots">
              {CASES.map((x, n) => (
                <button key={x.name} type="button" className={n === i ? 'on' : ''} onClick={() => go(n)} aria-label={`Show case ${n + 1}: ${x.name}`} aria-pressed={n === i} />
              ))}
            </div>
          </div>
        </div>
        <p className="kinds-label">Built for funds that hold money for others</p>
      </div>
      <div className="marquee slow" aria-hidden="true">
        {[0, 1].map((copy) => (
          <div key={copy} className="marquee-set">
            {KINDS.map((k) => (
              <span key={k} className="kind">
                {k}
              </span>
            ))}
          </div>
        ))}
      </div>
    </section>
  )
}

/* ---------------- funds on this registry ---------------- */

function Funds({ state }: { state: RegistryState | null }) {
  const [ref, seen] = useInView<HTMLElement>(0.1)
  const funds = state?.funds.slice(0, 6) ?? []
  return (
    <section id="funds" ref={ref} className="l-section bordered-top">
      <div className="l-wrap">
        <header className="l-head split">
          <div>
            <Label>On this registry</Label>
            <h2 className={`l-h2 reveal ${seen ? 'in' : ''}`}>
              Funds proving
              <br />
              their reserves.
            </h2>
          </div>
          {state && (
            <a className="btn quiet" href="#/funds">
              View all {state.funds.length} <Icon name="arrowRight" />
            </a>
          )}
        </header>
        {!state ? (
          <p className="muted">Connecting to the registry…</p>
        ) : funds.length === 0 ? (
          <p className="muted">
            No funds yet. <a href="#/list">Be the first to list one.</a>
          </p>
        ) : (
          <ul className="fund-tiles">
            {funds.map((f, i) => (
              <li key={f.id} className={`reveal ${seen ? 'in' : ''}`} style={{ transitionDelay: `${i * 60}ms` }}>
                <a href={fundHref(f.id)}>
                  <span className="ft-top">
                    <StatusDot status={f.status} />
                    <Icon name="arrowUpRight" />
                  </span>
                  <span className="ft-name">{f.name}</span>
                  <span className="ft-line">
                    {f.latest ? `≥ ${zec(f.latest.threshold_zat)} ${f.unit} · since block ${f.latest.since_height}` : f.status_text}
                  </span>
                </a>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  )
}

/* ---------------- call to action ---------------- */

function Cta() {
  const [ref, seen] = useInView<HTMLDivElement>(0.2)
  const [spot, setSpot] = useState({ x: 50, y: 50 })
  return (
    <section className="l-section">
      <div className="l-wrap">
        <div
          ref={ref}
          className={`cta-box reveal ${seen ? 'in' : ''}`}
          onMouseMove={(e) => {
            const r = e.currentTarget.getBoundingClientRect()
            setSpot({ x: ((e.clientX - r.left) / r.width) * 100, y: ((e.clientY - r.top) / r.height) * 100 })
          }}
          style={{ '--sx': `${spot.x}%`, '--sy': `${spot.y}%` } as React.CSSProperties}
        >
          <div className="cta-spot" aria-hidden="true" />
          <div className="cta-inner">
            <div className="cta-copy">
              <h2>
                Ready to prove
                <br />
                your reserve?
              </h2>
              <p>Free and open source. Your donors stay private, and your members can check the proof themselves.</p>
              <div className="hero-ctas">
                <a className="btn lg" href="#/list">
                  List your fund <Icon name="arrowRight" />
                </a>
                <a className="btn quiet lg" href="#/funds">
                  Verify a fund
                </a>
              </div>
              <p className="cta-note">No viewing key required</p>
            </div>
            <div className="cta-art">
              <AsciiCanvas shape="tetra" />
            </div>
          </div>
          <span className="corner tr" aria-hidden="true" />
          <span className="corner bl" aria-hidden="true" />
        </div>
      </div>
    </section>
  )
}

/* ---------------- footer ---------------- */

const FOOT = {
  Registry: [
    { name: 'All funds', href: '#/funds' },
    { name: 'List your fund', href: '#/list' },
  ],
  Learn: [
    { name: 'The proof', href: '#/?to=proof' },
    { name: 'How it works', href: '#/?to=how' },
    { name: 'Why not a viewing key', href: '#/?to=privacy' },
  ],
}

function Footer({ state }: { state: RegistryState | null }) {
  return (
    <footer className="l-foot">
      <div className="foot-wave" aria-hidden="true">
        <AsciiCanvas shape="wave" />
      </div>
      <div className="l-wrap">
        <div className="foot-grid">
          <div className="foot-brand">
            <a href="#/" className="logo">
              <span className="word">Z-Solvency</span>
              <span className="tm">ZEC</span>
            </a>
            <p>Private proof of reserves for funds on Zcash Ironwood. Show the money, not the people.</p>
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
              Every fund page has a Verify button, and <code>zsolv verify</code> checks any attestation on your own machine.
            </p>
          </div>
        </div>
        <div className="foot-bar">
          <p>Open source. No analytics, no third-party scripts.</p>
          <span className="live">
            <span className="live-dot" />
            {state ? `Registry online · ${state.network} · tip ${state.tip}` : 'Connecting to the registry'}
          </span>
        </div>
      </div>
    </footer>
  )
}
