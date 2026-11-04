import { useEffect, useState } from 'react'
import { api } from '../api'
import type { LatestAttestation, Verdict } from '../types'

const STAGGER_MS = 120

export function VerifyPanel({ link, fileName, latest }: { link: string; fileName: string; latest: LatestAttestation }) {
  const [verdict, setVerdict] = useState<Verdict | null>(null)
  const [shown, setShown] = useState(0)
  const [running, setRunning] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Reveal the checks one by one, so a viewer can follow what is being checked.
  useEffect(() => {
    if (!verdict || shown > verdict.checks.length) return
    const t = setTimeout(() => setShown((n) => n + 1), STAGGER_MS)
    return () => clearTimeout(t)
  }, [verdict, shown])

  async function run() {
    setRunning(true)
    setError(null)
    setVerdict(null)
    setShown(0)
    try {
      setVerdict(await api.verify(link))
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setRunning(false)
    }
  }

  const origin = window.location.origin
  return (
    <section className="card">
      <h2 className="eyebrow">Don't trust this page: verify</h2>
      <div className="row">
        <button onClick={run} disabled={running}>
          {running ? 'Verifying…' : verdict ? 'Verify again' : 'Verify now'}
        </button>
        <span className="muted">Re-checks both proofs against tree roots rebuilt from public chain data.</span>
      </div>
      {error && <p className="bad">{error}</p>}
      {verdict && (
        <ul className="checks">
          {verdict.checks.slice(0, shown).map((c) => (
            <li key={c.name}>
              <span className={`ic ${c.ok ? 'ok' : 'bad'}`}>{c.ok ? '✔' : '✘'}</span>
              <div>
                {c.name}
                <div className="detail">{c.detail}</div>
              </div>
            </li>
          ))}
          {shown > verdict.checks.length && (
            <li>
              <span className={`ic ${verdict.ok ? 'ok' : 'bad'}`}>{verdict.ok ? '✔' : '✘'}</span>
              <div>
                <strong>{verdict.ok ? 'Valid' : 'Invalid'}</strong>{' '}
                <span className="detail">checked against chain tip {verdict.tip}</span>
              </div>
            </li>
          )}
        </ul>
      )}
      <p className="muted spaced">
        Or on your own machine (
        <a href={api.attestationUrl(link)} download={`${fileName}.json`}>
          download the attestation
        </a>
        ):
      </p>
      <pre>
        {`curl -s ${origin}${api.attestationUrl(link)} > ${fileName}.json\n`}
        {`zsolv roots --height ${latest.at_height}\n`}
        {`zsolv verify ${fileName}.json`}
      </pre>
    </section>
  )
}
