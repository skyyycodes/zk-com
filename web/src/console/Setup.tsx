import { useState } from 'react'
import { consoleApi } from './api'
import type { Unconfigured } from './types'
import { Field, Problem, message } from './ui'

type Mode = 'create' | 'import' | 'existing'
type Network = 'devnet' | 'testnet'
const LOCAL_REGISTRY = 'http://127.0.0.1:8080'

/** `onWords` hands new backup words up to the app, which keeps them on screen until Continue (state polling would otherwise replace this screen). */
export function Setup({ state, onDone, onWords }: { state: Unconfigured; onDone: () => void; onWords: (words: string) => void }) {
  const [mode, setMode] = useState<Mode>('create')
  const [network, setNetwork] = useState<Network>(state.devnet_here ? 'devnet' : 'testnet')
  const [registry, setRegistry] = useState(state.devnet_here ? LOCAL_REGISTRY : '')
  const [name, setName] = useState('fund')
  const [existing, setExisting] = useState(state.wallets[0] ?? '')
  const [words, setWords] = useState('')
  const [birthday, setBirthday] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      const r = await consoleApi.setup({
        mode,
        network,
        registry_url: registry,
        wallet: mode === 'existing' ? existing : name,
        words: mode === 'import' ? words : undefined,
        birthday: mode === 'import' && birthday.trim() ? Number(birthday) : undefined,
      })
      if (r.words) onWords(r.words)
      else onDone()
    } catch (err) {
      setError(message(err))
    } finally {
      setBusy(false)
    }
  }

  const action = mode === 'create' ? 'Create wallet and fund key' : mode === 'import' ? 'Import wallet' : 'Open wallet'
  return (
    <form className="card form" onSubmit={submit} aria-busy={busy}>
      <h1>Set up your fund console</h1>
      <p className="desc">Everything you enter here stays on this laptop.</p>

      <fieldset>
        <legend>Wallet</legend>
        <div className="options">
          <Option checked={mode === 'create'} onChange={() => setMode('create')} title="Create a new fund wallet">
            You'll get 24 backup words. Write them down.
          </Option>
          <Option checked={mode === 'import'} onChange={() => setMode('import')} title="Import a wallet">
            From its backup words, as in Zkool or Zingo.
          </Option>
          {state.wallets.length > 0 && (
            <Option checked={mode === 'existing'} onChange={() => setMode('existing')} title="Open a wallet already here">
              {state.wallets.length} in this folder.
            </Option>
          )}
        </div>
      </fieldset>

      <fieldset>
        <legend>Network</legend>
        <div className="segmented">
          {(['devnet', 'testnet'] as const).map((n) => (
            <label key={n} className={network === n ? 'on' : ''}>
              <input
                type="radio"
                name="network"
                value={n}
                checked={network === n}
                onChange={() => {
                  setNetwork(n)
                  if (registry === '' || registry === LOCAL_REGISTRY) setRegistry(n === 'devnet' ? LOCAL_REGISTRY : '')
                }}
              />
              {n === 'devnet' ? 'Devnet' : 'Testnet'}
            </label>
          ))}
        </div>
        {network === 'devnet' && !state.devnet_here && (
          <p className="hint">The devnet console must run in the same folder as the devnet registry.</p>
        )}
      </fieldset>

      <Field label="Registry" hint="Where the fund's page is published.">
        {(p) => (
          <input
            {...p}
            type="url"
            inputMode="url"
            required
            spellCheck={false}
            autoComplete="url"
            placeholder="https://registry.example.org"
            value={registry}
            onChange={(e) => setRegistry(e.target.value)}
          />
        )}
      </Field>

      {mode === 'existing' ? (
        <Field label="Wallet">
          {(p) => (
            <select {...p} value={existing} onChange={(e) => setExisting(e.target.value)}>
              {state.wallets.map((w) => (
                <option key={w}>{w}</option>
              ))}
            </select>
          )}
        </Field>
      ) : (
        <Field label="Wallet name" hint="A label for the file on this laptop.">
          {(p) => (
            <input {...p} required spellCheck={false} autoComplete="off" pattern="[A-Za-z0-9_\-]+" value={name} onChange={(e) => setName(e.target.value)} />
          )}
        </Field>
      )}

      {mode === 'import' && (
        <>
          <Field label="Backup words" hint="12 or 24 words, separated by spaces.">
            {(p) => (
              <textarea {...p} required rows={3} spellCheck={false} autoComplete="off" value={words} onChange={(e) => setWords(e.target.value)} />
            )}
          </Field>
          {network === 'testnet' && (
            <Field label="Scan from block (optional)" hint="The wallet's birthday. Blank scans all of Ironwood, about a minute.">
              {(p) => (
                <input {...p} type="text" inputMode="numeric" pattern="[0-9]*" autoComplete="off" value={birthday} onChange={(e) => setBirthday(e.target.value)} />
              )}
            </Field>
          )}
        </>
      )}

      <Problem message={error} />
      <div className="row">
        <button type="submit" disabled={busy}>
          {busy ? 'Setting up…' : action}
        </button>
        <span className="muted small">Your fund key is derived from this wallet, so one backup restores both.</span>
      </div>
    </form>
  )
}

function Option({ checked, onChange, title, children }: { checked: boolean; onChange: () => void; title: string; children: React.ReactNode }) {
  return (
    <label className={`option ${checked ? 'on' : ''}`}>
      <input type="radio" name="wallet-mode" checked={checked} onChange={onChange} />
      <span>
        <strong>{title}</strong>
        <span className="muted small">{children}</span>
      </span>
    </label>
  )
}

export function BackupWords({ words, onContinue }: { words: string; onContinue: () => void }) {
  const [saved, setSaved] = useState(false)
  return (
    <section className="card form">
      <h1>Write down your backup words</h1>
      <p className="desc">
        These 24 words restore the fund's wallet and its fund key. Anyone who has them controls the fund's money. They
        won't be shown again.
      </p>
      <ol className="words">
        {words.split(/\s+/).map((w, i) => (
          <li key={i}>{w}</li>
        ))}
      </ol>
      <label className="check">
        <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
        I've written them down somewhere safe.
      </label>
      <div className="row">
        <button type="button" disabled={!saved} onClick={onContinue}>
          Continue
        </button>
      </div>
    </section>
  )
}
