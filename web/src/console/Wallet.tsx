import { useState } from 'react'
import { zec } from '../format'
import { consoleApi } from './api'
import type { MoveBody } from './api'
import type { Configured } from './types'
import { Field, Problem, Tile, message } from './ui'

export function Wallet({ state, reload }: { state: Configured; reload: () => void }) {
  const { wallet, unit } = state
  const [copied, setCopied] = useState(false)
  const reserve = wallet.reserve && wallet.reserve.spent_height == null ? wallet.reserve : null

  async function copy() {
    try {
      await navigator.clipboard.writeText(wallet.address)
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    } catch {
      setCopied(false)
    }
  }

  return (
    <>
      <section className="card">
        <h1>Wallet</h1>
        <div className="tiles">
          <Tile label="Balance" value={`${zec(wallet.balance_zat)} ${unit}`} sub={`${wallet.notes} notes`} />
          <Tile label="Operating money" value={`${zec(wallet.operating_zat)} ${unit}`} sub="What payouts may spend" />
          <Tile label="Reserve" value={reserve ? `${zec(reserve.value_zat)} ${unit}` : 'None'} sub={reserve ? `since block ${reserve.height}` : 'Lock it on Reserve'} />
        </div>
        <div className="field spaced">
          <span className="label">Receive address</span>
          <div className="row">
            <span className="address mono">{wallet.address}</span>
            <button type="button" className="ghost" onClick={copy}>
              {copied ? 'Copied' : 'Copy'}
            </button>
          </div>
          <p className="hint">Stays in this console; the public page never shows it. Data and keys: {state.data_dir}</p>
        </div>
        {state.fund && (
          <div className="field spaced">
            <span className="label">Fund key (public)</span>
            <span className="address mono">{state.fund.key}</span>
            <p className="hint">
              Derived from this wallet, so the backup words restore it. A registry with closed registration lists it to let fund “{state.fund.id}” in.
            </p>
          </div>
        )}
      </section>
      {state.devnet ? <DevnetTools unit={unit} reload={reload} /> : <TestnetSync reload={reload} />}
    </>
  )
}

function MoveForm({ title, action, labelName, defaultLabel, onSubmit, unit, hint }: { title: string; action: string; labelName: string; defaultLabel: string; unit: string; hint?: string; onSubmit: (b: MoveBody) => Promise<unknown> }) {
  const [b, setB] = useState<MoveBody>({ amount: '', label: defaultLabel, memo: '' })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [ok, setOk] = useState<string | null>(null)
  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    setOk(null)
    try {
      await onSubmit(b)
      setOk(`${action}: ${b.amount} ${unit}`)
      setB({ ...b, amount: '', memo: '' })
    } catch (err) {
      setError(message(err))
    } finally {
      setBusy(false)
    }
  }
  return (
    <form className="mini-form" onSubmit={submit} aria-busy={busy}>
      <h3>{title}</h3>
      {hint && <p className="hint">{hint}</p>}
      <div className="pair-fields">
        <Field label={`Amount (${unit})`}>
          {(p) => <input {...p} required type="text" inputMode="decimal" autoComplete="off" value={b.amount} onChange={(e) => setB({ ...b, amount: e.target.value })} placeholder="50" />}
        </Field>
        <Field label={labelName}>{(p) => <input {...p} autoComplete="off" value={b.label} onChange={(e) => setB({ ...b, label: e.target.value })} />}</Field>
      </div>
      <Field label="Memo">{(p) => <input {...p} autoComplete="off" value={b.memo} onChange={(e) => setB({ ...b, memo: e.target.value })} />}</Field>
      <Problem message={error} />
      {ok && (
        <p className="success" role="status">
          {ok}
        </p>
      )}
      <button type="submit" disabled={busy}>
        {busy ? 'Working…' : action}
      </button>
    </form>
  )
}

function DevnetTools({ unit, reload }: { unit: string; reload: () => void }) {
  const [blocks, setBlocks] = useState('10')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  async function mine(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await consoleApi.mine(Number(blocks) || 1)
      reload()
    } catch (err) {
      setError(message(err))
    } finally {
      setBusy(false)
    }
  }
  const after = (f: (b: MoveBody) => Promise<unknown>) => async (b: MoveBody) => {
    await f(b)
    reload()
  }
  return (
    <section className="card">
      <h2 className="eyebrow">Devnet tools</h2>
      <div className="tool-grid">
        <MoveForm title="Receive a test donation" action="Receive" labelName="From" defaultLabel="anonymous donor" unit={unit} onSubmit={after(consoleApi.receive)} />
        <MoveForm
          title="Pay from operating money"
          action="Pay"
          labelName="To"
          defaultLabel="members"
          unit={unit}
          hint="Refused if it would touch the reserve note."
          onSubmit={after(consoleApi.pay)}
        />
        <form className="mini-form" onSubmit={mine} aria-busy={busy}>
          <h3>Let time pass</h3>
          <p className="hint">Mine empty blocks on the devnet.</p>
          <Field label="Blocks">
            {(p) => <input {...p} type="text" inputMode="numeric" pattern="[0-9]*" autoComplete="off" value={blocks} onChange={(e) => setBlocks(e.target.value)} />}
          </Field>
          <Problem message={error} />
          <button type="submit" disabled={busy}>
            {busy ? 'Mining…' : 'Mine blocks'}
          </button>
        </form>
      </div>
    </section>
  )
}

function TestnetSync({ reload }: { reload: () => void }) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  async function sync() {
    setBusy(true)
    setError(null)
    try {
      await consoleApi.sync()
      reload()
    } catch (e) {
      setError(message(e))
    } finally {
      setBusy(false)
    }
  }
  return (
    <section className="card">
      <h2 className="eyebrow">Chain</h2>
      <p className="desc">Fetch new blocks from lightwalletd and find this wallet's notes. Payments go from your everyday wallet.</p>
      <Problem message={error} />
      <button type="button" onClick={sync} disabled={busy}>
        {busy ? 'Syncing…' : 'Sync now'}
      </button>
    </section>
  )
}
