import { blocks, short, usd, zec } from '../format'
import type { Fund, LatestAttestation } from '../types'
import { ContrastPanel } from './ContrastPanel'
import { StatusLight } from './StatusLight'
import { VerifyPanel } from './VerifyPanel'

export function FundPage({ fund }: { fund: Fund }) {
  const latest = fund.latest
  return (
    <>
      <section className="card">
        <h1>{fund.name}</h1>
        {fund.description && <p className="desc">{fund.description}</p>}
        <StatusBanner fund={fund} />
        {latest && fund.last_refusal && (
          <p className="notice">Latest submission refused: {fund.last_refusal}</p>
        )}
        {latest && <Facts fund={fund} latest={latest} />}
      </section>
      <Obligations fund={fund} />
      {latest && <VerifyPanel fundId={fund.id} latest={latest} />}
      <ContrastPanel fund={fund} />
    </>
  )
}

function StatusBanner({ fund }: { fund: Fund }) {
  const latest = fund.latest
  return (
    <div className={`status ${fund.status}`}>
      <StatusLight status={fund.status} />
      <div>
        <div className="claim">
          {latest
            ? `Holds at least ${zec(latest.threshold_zat)} ZEC`
            : fund.status === 'red'
              ? 'Reserve proof refused'
              : 'No reserve proof yet'}
          {latest && fund.zec_usd != null && (
            <small>
              ≈ ${usd(latest.threshold_zat, fund.zec_usd)} at ${fund.zec_usd}/ZEC
              {fund.price_time ? ` (${fund.price_time})` : ''}
            </small>
          )}
        </div>
        <div className="status-text">{fund.status_text}</div>
      </div>
    </div>
  )
}

function Fact({ k, v, sub, mono, title }: { k: string; v: string; sub: string; mono?: boolean; title?: string }) {
  return (
    <div className="fact">
      <div className="k">{k}</div>
      <div className={`v ${mono ? 'mono' : ''}`} title={title}>
        {v}
      </div>
      <div className="sub">{sub}</div>
    </div>
  )
}

function Facts({ fund, latest }: { fund: Fund; latest: LatestAttestation }) {
  const held = latest.at_height - latest.since_height
  return (
    <div className="facts">
      <Fact
        k="Held since"
        v={`block ${latest.since_height}`}
        sub={`${blocks(held)} before the check${fund.min_held_blocks ? ` · promised ${fund.min_held_blocks}` : ''}`}
      />
      <Fact
        k="Last checked at"
        v={`block ${latest.at_height}`}
        sub={latest.blocks_behind_tip === 0 ? 'the current tip' : `${blocks(latest.blocks_behind_tip)} behind tip`}
      />
      <Fact
        k="Registry nullifier"
        v={short(latest.registry_nullifier)}
        title={latest.registry_nullifier ?? undefined}
        sub="links this fund's own proofs only"
        mono
      />
      <Fact k="Round" v={String(latest.round)} sub={`${fund.rounds} published`} />
      <Fact
        k="Proof"
        v="2 × Halo 2"
        sub={`${(latest.proof_bytes / 1024).toFixed(1)} KiB · proved in ${(latest.prove_ms / 1000).toFixed(1)} s`}
      />
    </div>
  )
}

function Obligations({ fund }: { fund: Fund }) {
  const latest = fund.latest
  const covered = latest != null && latest.threshold_zat >= fund.obligations_zat
  return (
    <section className="card">
      <h2 className="eyebrow">Solvency: reserve vs. published obligations</h2>
      <table>
        <thead>
          <tr>
            <th>Obligation</th>
            <th className="num">ZEC</th>
          </tr>
        </thead>
        <tbody>
          {fund.obligations.map((o) => (
            <tr key={o.label}>
              <td>{o.label}</td>
              <td className="num">{zec(o.zat)}</td>
            </tr>
          ))}
          <tr className="total">
            <td>Total obligations</td>
            <td className="num">{zec(fund.obligations_zat)}</td>
          </tr>
          {latest && (
            <tr className="total">
              <td>Proven reserve (at least)</td>
              <td className={`num ${covered ? 'good' : 'bad'}`}>
                {zec(latest.threshold_zat)} {covered ? '✔' : '✘'}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  )
}
