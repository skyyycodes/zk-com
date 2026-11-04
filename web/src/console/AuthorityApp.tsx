import { useCallback, useEffect, useState } from 'react'
import { ago, zec } from '../format'
import { consoleApi } from './api'
import { CertificateDoc, roleName } from './CertificateDoc'
import type { AuthorityState, Certificate, CheckResult, Inbox, InboxItem } from './types'
import { Field, Problem, message } from './ui'

/** The console for an auditor, regulator or compliance body (`zsolv console --authority`). */
export function AuthorityApp({ state, reload }: { state: AuthorityState; reload: () => void }) {
  if (!state.configured) return <AuthoritySetup onDone={reload} />
  return <Review state={state} />
}

function AuthoritySetup({ onDone }: { onDone: () => void }) {
  const [name, setName] = useState('')
  const [role, setRole] = useState('auditor')
  const [url, setUrl] = useState(`http://${window.location.hostname}:8080`)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await consoleApi.authoritySetup({ name, role, registry_url: url })
      onDone()
    } catch (err) {
      setError(message(err))
    } finally {
      setBusy(false)
    }
  }
  return (
    <section className="card">
      <h1>Set up your authority console</h1>
      <p className="desc">
        For auditors, regulators and compliance bodies. You get a signing key that stays on this machine. Once the registry operator
        approves it, companies can ask you to certify their reserve proofs.
      </p>
      <form className="form-grid" onSubmit={submit}>
        <Field label="Name" hint="As it will appear on every certificate you sign.">
          {(p) => <input {...p} required value={name} maxLength={120} onChange={(e) => setName(e.target.value)} placeholder="Acme Audit LLP" />}
        </Field>
        <Field label="Role">
          {(p) => (
            <select {...p} value={role} onChange={(e) => setRole(e.target.value)}>
              <option value="auditor">Auditor</option>
              <option value="regulator">Regulator</option>
              <option value="compliance">Compliance body</option>
            </select>
          )}
        </Field>
        <Field label="Registry">
          {(p) => <input {...p} type="url" required value={url} onChange={(e) => setUrl(e.target.value)} />}
        </Field>
        <div className="row">
          <button type="submit" disabled={busy}>
            {busy ? 'Creating…' : 'Create signing key'}
          </button>
        </div>
        <Problem message={error} />
      </form>
    </section>
  )
}

function Review({ state }: { state: AuthorityState }) {
  const [inbox, setInbox] = useState<Inbox | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [open, setOpen] = useState<Certificate | null>(null)
  const load = useCallback(async () => {
    if (!state.approved) return
    try {
      setInbox(await consoleApi.inbox())
      setError(null)
    } catch (e) {
      setError(message(e))
    }
  }, [state.approved])
  useEffect(() => {
    void load()
    const t = setInterval(() => void load(), 6000)
    return () => clearInterval(t)
  }, [load])

  if (open) return <CertificateDoc c={open} unit={state.network === 'testnet' ? 'TAZ' : 'ZEC'} onClose={() => setOpen(null)} />

  const pending = inbox?.requests.filter((r) => r.request.status === 'pending') ?? []
  const done = inbox?.requests.filter((r) => r.request.status !== 'pending') ?? []

  return (
    <>
      <section className="card">
        <div className="row between">
          <div>
            <h1>{state.name}</h1>
            <p className="desc">
              {roleName(state.role)} on registry <span className="mono">{state.registry_id}</span>
            </p>
          </div>
          {state.approved ? <span className="pill green">Approved to certify</span> : <span className="pill amber">Waiting for approval</span>}
        </div>
        {!state.approved && (
          <>
            <p className="desc">
              Send your public key to the registry operator. They approve it with this command; this page updates by itself.
            </p>
            <pre className="cmd">
              zsolv registry approve-authority --key {state.key} --name "{state.name}" --role {state.role}
            </pre>
          </>
        )}
        <p className="hint">
          Public key <span className="mono">{state.key}</span>. The signing key stays in {state.data_dir}.
        </p>
      </section>

      <Problem message={error ?? state.registry_error ?? null} />

      {state.approved && (
        <section className="card">
          <h2 className="eyebrow">Requests to review</h2>
          {pending.length === 0 ? (
            <p className="desc">Nothing waiting. A company’s request shows up here as soon as it’s sent.</p>
          ) : (
            pending.map((item) => <RequestCard key={item.request.id} item={item} unit={state.network === 'testnet' ? 'TAZ' : 'ZEC'} registry={state.registry_url ?? ''} onDone={load} />)
          )}
        </section>
      )}

      {done.length > 0 && (
        <section className="card">
          <h2 className="eyebrow">Answered</h2>
          <table>
            <thead>
              <tr>
                <th>Company</th>
                <th>Round</th>
                <th>Decision</th>
                <th>When</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {done.map(({ request: r, fund_name }) => (
                <tr key={r.id}>
                  <td>{fund_name ?? r.fund_id}</td>
                  <td>{r.round}</td>
                  <td>
                    {r.status === 'issued' ? <span className="pill green">Certified</span> : <span className="pill red">Declined</span>}
                    {r.reason && <span className="cert-sub">{r.reason}</span>}
                  </td>
                  <td className="muted">{r.decided_at ? ago(r.decided_at) : ''}</td>
                  <td>
                    {r.certificate && (
                      <button type="button" className="ghost" onClick={() => setOpen(r.certificate)}>
                        View
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </>
  )
}

function RequestCard({ item, unit, registry, onDone }: { item: InboxItem; unit: string; registry: string; onDone: () => void }) {
  const r = item.request
  const a = item.attestation
  const [check, setCheck] = useState<CheckResult | null>(null)
  const [note, setNote] = useState(r.note)
  const [reason, setReason] = useState('')
  const [declining, setDeclining] = useState(false)
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function run(what: string, f: () => Promise<unknown>) {
    setBusy(what)
    setError(null)
    try {
      await f()
    } catch (e) {
      setError(message(e))
    } finally {
      setBusy(null)
    }
  }

  return (
    <article className="review">
      <div className="row between">
        <div>
          <h3>{item.fund_name ?? r.fund_id}</h3>
          {item.fund_description && <p className="hint">{item.fund_description}</p>}
        </div>
        <span className="muted">requested {ago(r.requested_at)}</span>
      </div>
      {a && (
        <p className="review-claim">
          Round {a.round}: at least <strong>{zec(a.threshold_zat)} {unit}</strong>, held since block {a.since_height}, checked at block {a.at_height}
          {item.obligations_zat != null && item.obligations_zat > 0 && (
            <span className="cert-sub">
              Published obligations: {zec(item.obligations_zat)} {unit}
            </span>
          )}
        </p>
      )}
      {r.note && <p className="cert-note">“{r.note}”</p>}
      {item.link && (
        <p className="hint">
          <a href={`${registry}/#/v/${item.link}`} target="_blank" rel="noreferrer">
            Open the company’s page
          </a>
        </p>
      )}

      <div className="row">
        <button type="button" className="ghost" disabled={busy != null} onClick={() => run('check', async () => setCheck(await consoleApi.check(r.id)))}>
          {busy === 'check' ? 'Verifying…' : 'Verify on this machine'}
        </button>
        {check && (
          <span className={check.verdict.ok ? 'pill green' : 'pill red'}>
            {check.verdict.checks.filter((c) => c.ok).length} of {check.verdict.checks.length} checks pass, {(check.ms / 1000).toFixed(1)} s
          </span>
        )}
      </div>
      {check && (
        <ul className="checks">
          {check.verdict.checks.map((c) => (
            <li key={c.name}>
              <span className={`ic ${c.ok ? 'ok' : 'bad'}`}>{c.ok ? '✓' : '✗'}</span>
              <span>
                {c.name}
                <span className="detail"> {c.detail}</span>
              </span>
            </li>
          ))}
        </ul>
      )}

      {check?.verdict.ok && !declining && (
        <div className="form-grid spaced">
          <Field label="Note on the certificate" hint="Optional. It becomes part of what you sign.">
            {(p) => <input {...p} value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} />}
          </Field>
          <div className="row">
            <button type="button" disabled={busy != null} onClick={() => run('issue', async () => { await consoleApi.issue(r.id, note); onDone() })}>
              {busy === 'issue' ? 'Signing…' : 'Sign and issue certificate'}
            </button>
            <button type="button" className="ghost" onClick={() => setDeclining(true)}>
              Decline
            </button>
          </div>
        </div>
      )}
      {(declining || (check && !check.verdict.ok)) && (
        <div className="form-grid spaced">
          <Field label="Reason for declining" hint="The company sees this.">
            {(p) => <input {...p} required value={reason} maxLength={500} onChange={(e) => setReason(e.target.value)} placeholder="The proof doesn’t verify against our chain data" />}
          </Field>
          <div className="row">
            <button type="button" disabled={busy != null || !reason.trim()} onClick={() => run('decline', async () => { await consoleApi.decline(r.id, reason); onDone() })}>
              {busy === 'decline' ? 'Declining…' : 'Decline request'}
            </button>
            {check?.verdict.ok && (
              <button type="button" className="ghost" onClick={() => setDeclining(false)}>
                Back
              </button>
            )}
          </div>
        </div>
      )}
      <Problem message={error} />
    </article>
  )
}
