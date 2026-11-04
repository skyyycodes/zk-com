import { useState } from 'react'
import { parseZec, span, zec } from '../format'
import { CopyLink } from '../components/CopyLink'
import { StatusDot } from '../components/StatusLight'
import { consoleApi } from './api'
import type { Configured } from './types'
import { Field, Problem, message } from './ui'

const DAY = 1152
const WEEK = 8064
const PROMISES = [
  { blocks: 20, label: '20 blocks', devnetOnly: true },
  { blocks: DAY, label: '1 day' },
  { blocks: WEEK, label: '1 week' },
  { blocks: 2 * WEEK, label: '2 weeks' },
  { blocks: 4 * WEEK, label: '4 weeks' },
]
const CADENCES = [
  { blocks: DAY, label: 'Day' },
  { blocks: WEEK, label: 'Week' },
  { blocks: 2 * WEEK, label: 'Two weeks' },
]

interface Row {
  label: string
  amount: string
}

const slug = (s: string) =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 64)

export function Register({ state, reload }: { state: Configured; reload: () => void }) {
  const reg = state.fund?.registry ?? null
  const registered = state.fund != null
  const [name, setName] = useState(reg?.name ?? '')
  const [id, setId] = useState(state.fund?.id ?? '')
  const [idTouched, setIdTouched] = useState(registered)
  const [description, setDescription] = useState(reg?.description ?? '')
  const [rows, setRows] = useState<Row[]>(reg?.obligations.length ? reg.obligations.map((o) => ({ label: o.label, amount: zec(o.zat).replace(/,/g, '') })) : [{ label: '', amount: '' }])
  const [promise, setPromise] = useState(reg?.min_held_blocks ?? (state.devnet ? 20 : 4 * WEEK))
  const [cadence, setCadence] = useState(reg?.max_proof_age_blocks || WEEK)
  const [busy, setBusy] = useState(false)
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [problem, setProblem] = useState<string | null>(null)
  const [done, setDone] = useState<string | null>(null)

  const pageId = idTouched ? id : slug(name)
  const amounts = rows.map((r) => (r.amount.trim() ? parseZec(r.amount) : null))
  const total = amounts.reduce<number>((t, a) => t + (a ?? 0), 0)
  const promises = PROMISES.filter((p) => !p.devnetOnly || state.devnet)
  if (!promises.some((p) => p.blocks === promise)) promises.push({ blocks: promise, label: span(promise) })
  const cadences = CADENCES.some((c) => c.blocks === cadence) ? CADENCES : [...CADENCES, { blocks: cadence, label: span(cadence) }]

  function validate(): Record<string, string> {
    const e: Record<string, string> = {}
    if (!name.trim()) e.name = 'Give the fund a name.'
    if (!/^[A-Za-z0-9-]{1,64}$/.test(pageId)) e.id = 'Use letters, digits and dashes.'
    rows.forEach((r, i) => {
      if (!r.label.trim() && !r.amount.trim()) return
      if (!r.label.trim()) e[`label${i}`] = 'Say what this obligation is.'
      if (amounts[i] == null || amounts[i] === 0) e[`amount${i}`] = `Enter an amount in ${state.unit}, like 480 or 12.5.`
    })
    if (total === 0) e.amount0 = e.amount0 ?? 'Add at least one obligation: the reserve covers them.'
    return e
  }

  async function submit(ev: React.FormEvent) {
    ev.preventDefault()
    const e = validate()
    setErrors(e)
    setProblem(null)
    if (Object.keys(e).length) {
      document.getElementById(`reg-${Object.keys(e)[0]}`)?.focus()
      return
    }
    setBusy(true)
    try {
      const obligations = rows
        .map((r, i) => ({ label: r.label.trim(), zat: amounts[i] ?? 0 }))
        .filter((o) => o.label && o.zat > 0)
      const r = await consoleApi.register({ id: pageId, name, description, obligations, min_held_blocks: promise, max_proof_age_blocks: cadence })
      setDone(r.page_url)
      reload()
    } catch (err) {
      setProblem(message(err))
    } finally {
      setBusy(false)
    }
  }

  const setRow = (i: number, patch: Partial<Row>) => setRows(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)))

  return (
    <form className="card form" onSubmit={submit} aria-busy={busy} noValidate>
      <h1>{registered ? 'Your fund page' : 'Register your fund'}</h1>
      <p className="desc">
        {registered ? 'Change what the page says. Every change is signed with your fund key.' : 'The page goes live in grey until your first proof.'}
      </p>
      <div className="split">
        <div className="form-col">
          <Field label="Fund name" error={errors.name} id="reg-name">
            {(p) => <input {...p} required autoComplete="off" value={name} onChange={(e) => setName(e.target.value)} placeholder="Northwind Custody" />}
          </Field>
          <Field
            label="Fund id"
            id="reg-id"
            error={errors.id}
            hint={
              registered
                ? 'Fixed once registered.'
                : 'Not public: your page is reached only through the link you get after saving. Saving again with the same id returns the same link.'
            }
          >
            {(p) => (
              <input
                {...p}
                required
                disabled={registered}
                spellCheck={false}
                autoComplete="off"
                value={pageId}
                onChange={(e) => {
                  setIdTouched(true)
                  setId(e.target.value)
                }}
                placeholder="mill-workers"
              />
            )}
          </Field>
          <Field label="What it's for">
            {(p) => <textarea {...p} rows={2} value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Shielded ZEC held for institutional clients." />}
          </Field>

          <fieldset>
            <legend>Obligations</legend>
            {rows.map((r, i) => (
              <div className="obligation" key={i}>
                <Field label={`Obligation ${i + 1}`} error={errors[`label${i}`]} id={`reg-label${i}`}>
                  {(p) => <input {...p} autoComplete="off" value={r.label} onChange={(e) => setRow(i, { label: e.target.value })} placeholder="Client balances" />}
                </Field>
                <Field label={state.unit} error={errors[`amount${i}`]} id={`reg-amount${i}`}>
                  {(p) => <input {...p} type="text" inputMode="decimal" autoComplete="off" value={r.amount} onChange={(e) => setRow(i, { amount: e.target.value })} placeholder="480" />}
                </Field>
                {rows.length > 1 && (
                  <button type="button" className="ghost icon" aria-label={`Remove obligation ${i + 1}`} onClick={() => setRows(rows.filter((_, j) => j !== i))}>
                    ×
                  </button>
                )}
              </div>
            ))}
            <div className="row between">
              <button type="button" className="ghost" onClick={() => setRows([...rows, { label: '', amount: '' }])}>
                + Add obligation
              </button>
              <span className="mono">
                Total {zec(total)} {state.unit}
              </span>
            </div>
          </fieldset>

          <div className="pair-fields">
            <Field label="Holding promise" hint={promise >= 48 ? `≈ ${promise.toLocaleString('en-US')} blocks` : undefined}>
              {(p) => (
                <select {...p} value={promise} onChange={(e) => setPromise(Number(e.target.value))}>
                  {promises.map((o) => (
                    <option key={o.blocks} value={o.blocks}>
                      {o.label}
                    </option>
                  ))}
                </select>
              )}
            </Field>
            <Field label="Proof every">
              {(p) => (
                <select {...p} value={cadence} onChange={(e) => setCadence(Number(e.target.value))}>
                  {cadences.map((o) => (
                    <option key={o.blocks} value={o.blocks}>
                      {o.label}
                    </option>
                  ))}
                </select>
              )}
            </Field>
          </div>
        </div>

        <aside className="preview" aria-label="Preview">
          <div className="eyebrow">Preview</div>
          <div className="preview-card">
            <div className="row">
              <StatusDot status={reg?.status ?? 'none'} />
              <strong>{name || 'Your fund'}</strong>
            </div>
            <p className="muted small">{reg?.status_text ?? 'No proof published yet'}</p>
            <div className="preview-line">
              <span>Obligations</span>
              <span className="mono">
                {zec(total)} {state.unit}
              </span>
            </div>
            <div className="preview-line">
              <span>Holding promise</span>
              <span>{span(promise)}</span>
            </div>
          </div>
        </aside>
      </div>

      <Problem message={problem} />
      {done && (
        <div className="share-card" role="status">
          <p>
            Saved. This is your fund’s link: it isn’t listed anywhere, so only people you share it with can see the
            fund. It shows a proof once you publish one.
          </p>
          <CopyLink url={done} label="Your fund’s link" />
          <a className="next" href="#/reserve">
            Next: lock the reserve
          </a>
        </div>
      )}
      <div className="row">
        <button type="submit" disabled={busy}>
          {busy ? 'Publishing…' : registered ? 'Update fund page' : 'Publish fund page'}
        </button>
        <span className="muted small">Signed with your fund key</span>
      </div>
    </form>
  )
}
