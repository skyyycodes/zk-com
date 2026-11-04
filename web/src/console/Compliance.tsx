import { useCallback, useEffect, useState } from 'react'
import { ago, zec } from '../format'
import { consoleApi } from './api'
import { CertificateDoc, download, roleName } from './CertificateDoc'
import type { Certificate, Compliance as Data, Configured } from './types'
import { Field, Problem, message } from './ui'

const STEPS = [
  ['Publish a round', 'The registry checks your proof and certifies the round automatically. That certificate is yours alone.'],
  ['Ask an auditor or regulator', 'Choose an authority the registry operator has approved, and the round you want certified.'],
  ['They check it themselves', 'The authority re-verifies your proof on its own machine against its own copy of the chain, then signs a certificate or declines with a reason.'],
  ['Hand over the certificate', 'Download it or save it as a PDF. Anyone can check it with zsolv certificate verify; it reveals no balance, addresses or transactions.'],
] as const

export function Compliance({ state }: { state: Configured }) {
  const [data, setData] = useState<Data | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [open, setOpen] = useState<Certificate | null>(null)

  const load = useCallback(async () => {
    try {
      setData(await consoleApi.compliance())
      setError(null)
    } catch (e) {
      setError(message(e))
    }
  }, [])
  useEffect(() => {
    void load()
    const t = setInterval(() => void load(), 6000)
    return () => clearInterval(t)
  }, [load])

  if (!state.fund) {
    return (
      <section className="card">
        <h1>Compliance</h1>
        <p className="desc">
          Register the fund first. <a href="#/register">Register</a>
        </p>
      </section>
    )
  }

  const issued = (data?.requests ?? []).filter((r) => r.request.certificate).map((r) => r.request.certificate as Certificate)
  const certificates = [...issued, ...(data?.registry_certificates ?? [])]
  const open_or_declined = (data?.requests ?? []).filter((r) => r.request.status !== 'issued')

  if (open) {
    return <CertificateDoc c={open} unit={data?.unit ?? state.unit} onClose={() => setOpen(null)} />
  }

  return (
    <>
      <section className="card">
        <h1>Compliance</h1>
        <p className="desc">
          Auditors, regulators and compliance bodies certify your reserve without seeing your books. Each certificate covers one
          published round and stays private to you until you hand it over.
        </p>
        <ol className="flow">
          {STEPS.map(([t, d]) => (
            <li key={t}>
              <strong>{t}</strong>
              <span>{d}</span>
            </li>
          ))}
        </ol>
      </section>

      <Problem message={error} />

      {data && <RequestForm data={data} onDone={load} />}

      <section className="card">
        <h2 className="eyebrow">Certificates</h2>
        {certificates.length === 0 ? (
          <p className="desc">
            None yet. Publishing a round gets you the registry’s certificate. <a href="#/prove">Prove and publish</a>
          </p>
        ) : (
          <table>
            <thead>
              <tr>
                <th>Issued by</th>
                <th>Round</th>
                <th className="num">At least</th>
                <th>Issued</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {certificates.map((c) => (
                <tr key={c.id}>
                  <td>
                    {c.issuer_name}
                    <span className="cert-sub">{roleName(c.issuer_role)}</span>
                  </td>
                  <td>{c.round}</td>
                  <td className="num">
                    {zec(c.threshold_zat)} {data?.unit}
                  </td>
                  <td className="muted">{ago(c.issued_at)}</td>
                  <td>
                    <span className="row tight end">
                      <button type="button" className="ghost" onClick={() => setOpen(c)}>
                        View
                      </button>
                      <button type="button" className="ghost" onClick={() => download(c)}>
                        Download
                      </button>
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      {open_or_declined.length > 0 && (
        <section className="card">
          <h2 className="eyebrow">Requests</h2>
          <table>
            <thead>
              <tr>
                <th>Sent to</th>
                <th>Round</th>
                <th>Status</th>
                <th>Sent</th>
              </tr>
            </thead>
            <tbody>
              {open_or_declined.map(({ request: r, authority_name, authority_role }) => (
                <tr key={r.id}>
                  <td>
                    {authority_name ?? 'An authority no longer approved'}
                    <span className="cert-sub">{roleName(authority_role)}</span>
                  </td>
                  <td>{r.round}</td>
                  <td>
                    {r.status === 'pending' ? (
                      <span className="pill amber">Waiting for review</span>
                    ) : (
                      <>
                        <span className="pill red">Declined</span>
                        {r.reason && <span className="cert-sub">{r.reason}</span>}
                      </>
                    )}
                  </td>
                  <td className="muted">{ago(r.requested_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </>
  )
}

function RequestForm({ data, onDone }: { data: Data; onDone: () => void }) {
  const [authority, setAuthority] = useState('')
  const [round, setRound] = useState<string>('')
  const [note, setNote] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [sent, setSent] = useState<string | null>(null)
  const chosen = authority || data.authorities[0]?.key || ''
  const latest = data.rounds.length ? Math.max(...data.rounds) : null

  if (data.authorities.length === 0) {
    return (
      <section className="card">
        <h2 className="eyebrow">Ask for certification</h2>
        <p className="desc">
          No auditors or regulators are approved on this registry yet. The registry operator approves them with{' '}
          <code>zsolv registry approve-authority</code>.
        </p>
      </section>
    )
  }

  async function send(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    setSent(null)
    try {
      const r = await consoleApi.requestCertificate({ authority_key: chosen, round: round ? Number(round) : undefined, note })
      const a = data.authorities.find((x) => x.key === chosen)
      setSent(`Sent round ${r.round} to ${a?.name ?? 'the authority'}. It shows up here once they answer.`)
      setNote('')
      onDone()
    } catch (err) {
      setError(message(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <section className="card">
      <h2 className="eyebrow">Ask for certification</h2>
      {latest == null ? (
        <p className="desc">
          Publish a round first: there is nothing to certify yet. <a href="#/prove">Prove and publish</a>
        </p>
      ) : (
        <form className="form-grid" onSubmit={send}>
          <Field label="Auditor or regulator">
            {(p) => (
              <select {...p} value={chosen} onChange={(e) => setAuthority(e.target.value)}>
                {data.authorities.map((a) => (
                  <option key={a.key} value={a.key}>
                    {a.name} ({roleName(a.role)})
                  </option>
                ))}
              </select>
            )}
          </Field>
          <Field label="Round">
            {(p) => (
              <select {...p} value={round || String(latest)} onChange={(e) => setRound(e.target.value)}>
                {[...data.rounds].reverse().map((n) => (
                  <option key={n} value={n}>
                    Round {n}
                    {n === latest ? ' (latest)' : ''}
                  </option>
                ))}
              </select>
            )}
          </Field>
          <Field label="Note for the authority" hint="Optional, for example the reporting period. They see it with the request.">
            {(p) => <input {...p} value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} placeholder="Q4 2026 reserve review" />}
          </Field>
          <div className="row">
            <button type="submit" disabled={busy}>
              {busy ? 'Sending…' : 'Request certification'}
            </button>
          </div>
          <Problem message={error} />
          {sent && (
            <p className="hint" role="status">
              {sent}
            </p>
          )}
        </form>
      )}
    </section>
  )
}
