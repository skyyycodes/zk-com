import { ago } from '../format'
import { StatusDot } from '../components/StatusLight'
import type { Configured } from './types'

export function History({ state }: { state: Configured }) {
  const past = [...(state.fund?.registry?.past ?? [])].reverse()
  return (
    <section className="card">
      <h1>Past rounds</h1>
      {state.pending.length === 0 && past.length === 0 ? (
        <p className="desc">
          No rounds yet. Your first proof shows up here. <a href="#/prove">Prove and publish</a>
        </p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Round</th>
              <th className="num">Checked at block</th>
              <th>Result</th>
              <th>When</th>
            </tr>
          </thead>
          <tbody>
            {state.pending.map((p) => (
              <tr key={p.file}>
                <td>{p.round}</td>
                <td className="num">{p.at_height}</td>
                <td>Saved, not published yet</td>
                <td />
              </tr>
            ))}
            {past.map((p, i) => (
              <tr key={`${p.time}-${i}`}>
                <td>{p.round ?? '—'}</td>
                <td className="num">{p.at_height ?? '—'}</td>
                <td>
                  <span className="row tight">
                    <StatusDot status={p.light} />
                    {p.accepted ? p.text : `Refused: ${p.text}`}
                  </span>
                </td>
                <td className="muted">{ago(p.time)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  )
}
