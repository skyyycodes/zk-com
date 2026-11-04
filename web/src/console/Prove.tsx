import { useEffect, useState } from 'react'
import { zec } from '../format'
import { consoleApi } from './api'
import type { Configured, Job, Step } from './types'
import { Problem, message } from './ui'

const STEP = 12_500_000

export function Prove({ state, reload }: { state: Configured; reload: () => void }) {
  const [job, setJob] = useState<Job>(state.job)
  const [error, setError] = useState<string | null>(null)
  const [retrying, setRetrying] = useState(false)
  const { unit, wallet } = state
  const fund = state.fund?.registry ?? null
  const reserve = wallet.reserve && wallet.reserve.spent_height == null ? wallet.reserve : null

  // While a proof runs, follow it closely; the console's state poll is slower.
  useEffect(() => {
    if (!job.running) return
    const t = setInterval(async () => {
      try {
        const j = await consoleApi.job()
        setJob(j)
        if (!j.running) reload()
      } catch (e) {
        setError(message(e))
      }
    }, 400)
    return () => clearInterval(t)
  }, [job.running, reload])

  async function start() {
    setError(null)
    try {
      await consoleApi.prove()
      setJob(await consoleApi.job())
    } catch (e) {
      setError(message(e))
    }
  }

  async function retry() {
    setRetrying(true)
    setError(null)
    try {
      const r = await consoleApi.retry()
      const refused = r.results.filter((x) => !x.accepted)
      if (refused.length) setError(`Refused: ${refused.map((x) => x.reason).join('; ')}`)
      reload()
    } catch (e) {
      setError(message(e))
    } finally {
      setRetrying(false)
    }
  }

  if (!state.fund || !reserve) {
    return (
      <section className="card">
        <h1>This week's proof</h1>
        <p className="desc">
          {!state.fund ? (
            <>
              Register the fund first. <a href="#/register">Register</a>
            </>
          ) : (
            <>
              Lock the reserve first. <a href="#/reserve">Lock the reserve</a>
            </>
          )}
        </p>
      </section>
    )
  }

  const t = Math.floor(reserve.value_zat / STEP) * STEP
  const round = Math.max(fund?.latest?.round ?? 0, ...state.pending.map((p) => p.round)) + 1
  const result = job.result
  return (
    <section className="card">
      <h1>This week's proof</h1>
      <p className="desc">
        Next: round {round} · chain tip is block {state.tip ?? '…'}
      </p>
      <div className="claim-box mono">
        At least {zec(t)} {unit} · held since block {reserve.height} · checked at block {state.tip ?? '…'}
      </div>
      <div className="cols">
        <div className="col out">
          <strong>Goes to the registry</strong>
          <ul>
            <li>
              {zec(t)} {unit} and the two block heights
            </li>
            <li>5 registry nullifiers</li>
            <li>2 proofs, 3 signatures</li>
          </ul>
        </div>
        <div className="col in">
          <strong>Stays on this laptop</strong>
          <ul>
            <li>
              Balance: {zec(wallet.balance_zat)} {unit}
            </li>
            <li>Which notes, donors, memos</li>
            <li>Spending key</li>
          </ul>
        </div>
      </div>

      {state.pending.length > 0 && !job.running && (
        <p className="notice warn">
          {state.pending.length === 1 ? `Round ${state.pending[0].round} is saved` : `${state.pending.length} rounds are saved`} but not published yet.{' '}
          <button type="button" className="ghost" onClick={retry} disabled={retrying}>
            {retrying ? 'Retrying…' : 'Retry'}
          </button>
        </p>
      )}
      <Problem message={error} />
      <div className="row">
        <button type="button" onClick={start} disabled={job.running} aria-busy={job.running}>
          {job.running ? 'Proving…' : 'Prove and publish'}
        </button>
      </div>

      {job.id > 0 && (
        <ol className="steps" aria-live="polite">
          {job.steps.map((s, i) => (
            <StepRow key={i} step={s} />
          ))}
        </ol>
      )}
      {!job.running && result && (
        <p className={result.accepted ? 'success' : 'notice'} role="status">
          {result.accepted ? (
            <>
              Published round {result.round}. Your page is {result.status === 'green' ? 'green' : result.status === 'amber' ? 'amber for now' : 'updated'}.{' '}
              <a href={result.page_url}>{result.page_url.replace(/^https?:\/\//, '')}</a>
            </>
          ) : result.saved ? (
            <>Saved round {result.round}. {result.reason}</>
          ) : (
            <>Refused: {result.reason}</>
          )}
        </p>
      )}
      {!job.running && job.error && (
        <p className="notice" role="alert">
          {job.error}
        </p>
      )}
    </section>
  )
}

function StepRow({ step }: { step: Step }) {
  const icon = { done: '✓', failed: '✕', running: '…', pending: '·' }[step.state]
  return (
    <li className={step.state}>
      <span className="ic" aria-hidden="true">
        {icon}
      </span>
      <span>
        {step.label}
        {step.detail && <span className="detail">{step.detail}</span>}
      </span>
      <span className="t">{step.ms != null ? `${(step.ms / 1000).toFixed(step.ms < 100 ? 2 : 1)} s` : step.state === 'running' ? 'running' : ''}</span>
      <span className="sr-only">{step.state}</span>
    </li>
  )
}
