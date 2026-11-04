import { useEffect, useState } from 'react'
import { api } from '../api'
import { zec } from '../format'
import type { Fund, ViewingKeyExport } from '../types'

export function ContrastPanel({ fund }: { fund: Fund }) {
  const [vk, setVk] = useState<ViewingKeyExport | null | undefined>(undefined)
  const latest = fund.latest

  useEffect(() => {
    let live = true
    api.viewingKey(fund.id).then((v) => live && setVk(v), () => live && setVk(null))
    return () => {
      live = false
    }
  }, [fund.id])

  return (
    <section className="card">
      <h2 className="eyebrow">What a viewing key would have leaked</h2>
      <div className="contrast">
        <div className="leak">
          <h3>If the fund published its viewing key</h3>
          {vk === undefined && <p className="muted">Loading…</p>}
          {vk === null && <p className="muted">No demo export for this fund.</p>}
          {vk && (
            <>
              <div className="balance">Exact balance: {zec(vk.balance_zat)} {fund.unit}</div>
              <div className="scroll" tabIndex={0} role="region" aria-label="Viewing key history">
                <table>
                  <tbody>
                    {[...vk.history].reverse().map((h, i) => (
                      <tr key={i}>
                        <td>#{h.height}</td>
                        <td>
                          {h.direction === 'in' ? 'from' : 'to'} <strong>{h.counterparty}</strong>
                        </td>
                        <td className="num">
                          {h.direction === 'in' ? '+' : '−'}
                          {zec(h.value_zat)}
                        </td>
                        <td>
                          <em>{h.memo}</em>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <p className="warn">Permanent and exact. Every donor, recipient, memo and date, for anyone, forever.</p>
            </>
          )}
        </div>
        <div className="safe">
          <h3>What Z-Solvency publishes</h3>
          <div className="line">
            {latest
              ? `≥ ${zec(latest.threshold_zat)} ${fund.unit} · held since block ${latest.since_height} · checked at block ${latest.at_height} · signed by the fund`
              : '—'}
          </div>
          <p className="muted small">
            Not the balance, the notes, addresses, history, memos, donors or recipients. A new proof each round; nothing
            permanent to take back.
          </p>
        </div>
      </div>
    </section>
  )
}
