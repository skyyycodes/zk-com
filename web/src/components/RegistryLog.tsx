import type { Fund, RegistryEvent } from '../types'

export function RegistryLog({ events, funds }: { events: RegistryEvent[]; funds: Fund[] }) {
  const name = (id: string) => funds.find((f) => f.id === id)?.name ?? id
  return (
    <section className="log" aria-label="Registry log">
      <h2 className="eyebrow">Registry log</h2>
      {events.length === 0 && <p className="muted">Nothing yet.</p>}
      {events.slice(0, 12).map((e, i) => (
        <div className="event" key={`${e.time}-${i}`}>
          <span className={`pill ${e.accepted ? 'ok' : 'no'}`}>{e.accepted ? 'accepted' : 'refused'}</span>
          <div>
            <strong>{name(e.fund_id)}</strong>
            <div className="muted">{e.summary}</div>
          </div>
        </div>
      ))}
    </section>
  )
}
