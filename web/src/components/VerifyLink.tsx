import { useState } from 'react'
import { linkToken } from '../api'
import { linkHref } from '../hooks'

/** Paste a share link (or its token) to open the fund it points to. */
export function VerifyLink() {
  const [value, setValue] = useState('')
  const [error, setError] = useState<string | null>(null)
  function open(e: React.FormEvent) {
    e.preventDefault()
    const t = linkToken(value)
    if (!t) {
      setError('That isn’t a Z-Solvency link. It looks like …/#/v/ followed by 32 letters and digits.')
      return
    }
    window.location.hash = linkHref(t)
  }
  return (
    <>
      <section className="hero">
        <h1>Verify a fund</h1>
        <p className="desc">Paste the link a fund shared with you. You’ll see what it proves, and you can check the proof yourself.</p>
      </section>
      <form className="card form" onSubmit={open} noValidate>
        <div className="field">
          <label htmlFor="link">Fund link</label>
          <input
            id="link"
            type="url"
            inputMode="url"
            autoComplete="off"
            spellCheck={false}
            placeholder={`${window.location.origin}/#/v/…`}
            value={value}
            onChange={(e) => {
              setValue(e.target.value)
              setError(null)
            }}
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? 'link-error' : undefined}
          />
          {error && (
            <p className="field-error" id="link-error">
              {error}
            </p>
          )}
        </div>
        <div className="row">
          <button type="submit">Open the fund</button>
          <span className="muted small">Don’t have a link? Ask the fund for one.</span>
        </div>
      </form>
    </>
  )
}
