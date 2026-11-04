import { useState } from 'react'

/** A share link with a Copy button; says "Copied" for a moment after. */
export function CopyLink({ url, label = 'Share link' }: { url: string; label?: string }) {
  const [copied, setCopied] = useState(false)
  async function copy() {
    try {
      await navigator.clipboard.writeText(url)
    } catch {
      // Clipboard blocked (e.g. plain http in some browsers): select the text instead.
      const el = document.getElementById('share-url') as HTMLInputElement | null
      el?.select()
      return
    }
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }
  return (
    <div className="copy-link">
      <label htmlFor="share-url" className="copy-label">
        {label}
      </label>
      <div className="copy-row">
        <input id="share-url" readOnly value={url} onFocus={(e) => e.currentTarget.select()} spellCheck={false} />
        <button type="button" onClick={copy}>
          {copied ? 'Copied' : 'Copy link'}
        </button>
      </div>
      <span className="sr-only" role="status">
        {copied ? 'Link copied' : ''}
      </span>
    </div>
  )
}
