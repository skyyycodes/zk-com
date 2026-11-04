import { useState } from 'react'
import { span, zec } from '../format'
import { StatusDot } from '../components/StatusLight'
import { consoleApi } from './api'
import type { Configured } from './types'
import { Problem, Tile, message } from './ui'

const STEP = 12_500_000

export function Reserve({ state, reload }: { state: Configured; reload: () => void }) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const fund = state.fund?.registry ?? null
  const { unit, wallet } = state
  const reserve = wallet.reserve
  const live = reserve && reserve.spent_height == null ? reserve : null

  if (!state.fund) {
    return (
      <section className="card">
        <h1>Lock the reserve</h1>
        <p className="desc">
          Register the fund first: the reserve covers its obligations. <a href="#/register">Register</a>
        </p>
      </section>
    )
  }
  const obligations = fund?.obligations_zat ?? 0
  const target = Math.ceil(obligations / STEP) * STEP

  async function lock() {
    setBusy(true)
    setError(null)
    try {
      await consoleApi.reserve()
      reload()
    } catch (e) {
      setError(message(e))
    } finally {
      setBusy(false)
    }
  }

  const testnet = !state.devnet
  const action = testnet ? 'Find my reserve' : `Lock ${zec(target)} ${unit} as reserve`
  const held = live && state.tip != null ? state.tip - live.height : 0
  const promise = fund?.min_held_blocks ?? 0
  const kept = live && live.value_zat === target

  return (
    <section className="card">
      <h1>Lock the reserve</h1>
      <div className="tiles">
        <Tile label="Operating money" value={`${zec(wallet.operating_zat)} ${unit}`} sub="Pays strike pay and costs" />
        <Tile label="Reserve" value={live ? `${zec(live.value_zat)} ${unit}` : 'None yet'} sub={live ? `since block ${live.height}` : 'One note of exactly T'} />
      </div>

      {obligations === 0 ? (
        <p className="spaced">
          Add the fund's obligations first. <a href="#/register">Edit the fund page</a>
        </p>
      ) : kept ? (
        <div className="after">
          <div className="row">
            <StatusDot status={held >= promise ? 'green' : 'amber'} />
            <strong>Reserve locked at block {live.height}</strong>
          </div>
          {promise > 0 && (
            <>
              <div className="bar" role="progressbar" aria-valuemin={0} aria-valuemax={promise} aria-valuenow={Math.min(held, promise)} aria-label="Held for the promised time">
                <span style={{ width: `${Math.min(100, (held / promise) * 100)}%` }} />
              </div>
              <p className="muted small">
                {held >= promise
                  ? `Held ${span(held)}, past the ${span(promise)} promised.`
                  : `Held ${span(held)} of the ${span(promise)} promised. Green in about ${span(promise - held)}.`}
              </p>
            </>
          )}
          <a className="next" href="#/prove">
            Next: prove and publish
          </a>
        </div>
      ) : (
        <>
          {reserve?.spent_height != null && (
            <p className="notice" role="alert">
              Your reserve note was spent at block {reserve.spent_height}. Lock a new reserve; “held since” restarts.
            </p>
          )}
          {live && live.value_zat !== target && (
            <p className="notice warn">
              Your obligations total {zec(obligations)} {unit}; your reserve holds {zec(live.value_zat)}.{' '}
              {live.value_zat < target ? `Lock ${zec(target - live.value_zat)} ${unit} more.` : `Lock exactly ${zec(target)} ${unit}.`}{' '}
              “Held since” moves to that block.
            </p>
          )}
          <p className="spaced">
            Your obligations total {zec(obligations)} {unit}. Lock exactly {target === obligations ? 'that much' : `${zec(target)} ${unit}`} in one note sent to
            yourself, and pay out from the rest.
            {target !== obligations && ' (The proof counts in eighths of a coin, so T rounds up.)'}
          </p>
          {testnet && (
            <p className="hint">
              From your everyday wallet (Zkool, Zingo), send exactly {zec(target)} TAZ to this fund's own address, then press{' '}
              {action} once it has confirmed:
              <span className="address mono">{wallet.address}</span>
            </p>
          )}
          <Problem message={error} />
          <div className="row">
            <button type="button" onClick={lock} disabled={busy}>
              {busy ? (testnet ? 'Looking…' : 'Locking…') : action}
            </button>
          </div>
        </>
      )}
    </section>
  )
}
