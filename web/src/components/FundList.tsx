import { zec } from '../format'
import { fundHref } from '../hooks'
import type { Fund } from '../types'
import { StatusDot } from './StatusLight'

export function FundList({ funds, selectedId }: { funds: Fund[]; selectedId: string | null }) {
  return (
    <nav aria-label="Funds">
      <h3 className="eyebrow">Funds in this registry</h3>
      {funds.length === 0 && <p className="muted">No funds registered.</p>}
      {funds.map((f) => (
        <a key={f.id} href={fundHref(f.id)} className={`fund-item ${f.id === selectedId ? 'active' : ''}`}>
          <StatusDot status={f.status} />
          <span>
            <span className="fund-name">{f.name}</span>
            <span className="fund-sub">
              {f.latest
                ? `≥ ${zec(f.latest.threshold_zat)} ${f.unit} · since block ${f.latest.since_height}`
                : f.last_refusal
                  ? 'submission refused'
                  : 'no proof yet'}
            </span>
          </span>
        </a>
      ))}
    </nav>
  )
}
