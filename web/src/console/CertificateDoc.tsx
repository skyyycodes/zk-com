import { ago, short, zec } from '../format'
import type { Certificate } from './types'

const ROLE: Record<string, string> = { registry: 'Registry', auditor: 'Auditor', regulator: 'Regulator', compliance: 'Compliance body' }

export const roleName = (r: string | null | undefined) => (r ? (ROLE[r] ?? r) : '')

/** Save a certificate as a JSON file the recipient can check with `zsolv certificate verify`. */
export function download(c: Certificate) {
  const blob = new Blob([JSON.stringify(c, null, 2)], { type: 'application/json' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = `z-solvency-certificate-${c.fund_id}-round-${c.round}-${c.id}.json`
  a.click()
  setTimeout(() => URL.revokeObjectURL(a.href), 1000)
}

/** The certificate as a document: readable on screen, printable to PDF. */
export function CertificateDoc({ c, unit, onClose }: { c: Certificate; unit: string; onClose?: () => void }) {
  const issued = new Date(Number(c.issued_at) * 1000)
  return (
    <section className="card cert-doc" aria-label={`Certificate ${c.id}`}>
      <div className="row between no-print">
        <span className="hint">Certificate {c.id}</span>
        <span className="row tight">
          <button type="button" className="ghost" onClick={() => download(c)}>
            Download file
          </button>
          <button type="button" className="ghost" onClick={() => window.print()}>
            Print or save as PDF
          </button>
          {onClose && (
            <button type="button" className="ghost" onClick={onClose}>
              Close
            </button>
          )}
        </span>
      </div>

      <p className="cert-kind">{c.kind === 'registry' ? 'Registry certificate of reserve' : 'Certificate of reserve'}</p>
      <h2 className="cert-fund">{c.fund_name}</h2>
      <p className="cert-claim">
        held at least <strong>{zec(c.threshold_zat)} {unit}</strong> in the Zcash shielded pool from block {c.since_height} to block{' '}
        {c.at_height}, as stated in its round {c.round} proof.
      </p>

      <dl className="cert-grid">
        <div>
          <dt>Issued by</dt>
          <dd>
            {c.issuer_name}
            <span className="cert-sub">{roleName(c.issuer_role)}</span>
          </dd>
        </div>
        <div>
          <dt>Issued</dt>
          <dd>
            {issued.toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' })}
            <span className="cert-sub">{ago(c.issued_at)}</span>
          </dd>
        </div>
        <div>
          <dt>Proof checks</dt>
          <dd>
            {c.checks_passed} of {c.checks_total} passed
            <span className="cert-sub">{c.kind === 'registry' ? 'checked by the registry' : 'checked on the issuer’s own machine'}</span>
          </dd>
        </div>
        <div>
          <dt>Registry</dt>
          <dd className="mono">{c.registry}</dd>
        </div>
      </dl>

      {c.note && <p className="cert-note">“{c.note}”</p>}

      <dl className="cert-keys">
        <div>
          <dt>Statement hash</dt>
          <dd className="mono">{c.statement_hash}</dd>
        </div>
        <div>
          <dt>Block {c.at_height} hash</dt>
          <dd className="mono">{c.at_block_hash}</dd>
        </div>
        <div>
          <dt>Holder key</dt>
          <dd className="mono">{c.fund_key}</dd>
        </div>
        <div>
          <dt>Issuer key</dt>
          <dd className="mono">{c.issuer_key}</dd>
        </div>
        <div>
          <dt>Signature</dt>
          <dd className="mono">{short(c.signature)}</dd>
        </div>
      </dl>

      <p className="hint cert-how">
        This certificate says the holder’s zero-knowledge proof verified; it reveals no balance, addresses or transactions. Check it
        yourself: <code>zsolv certificate verify certificate.json --url &lt;registry&gt;</code>
      </p>
    </section>
  )
}
