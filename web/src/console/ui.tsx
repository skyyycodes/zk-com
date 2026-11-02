import { useId } from 'react'

/** A labelled form field with hint and inline error. */
export function Field({
  label,
  hint,
  error,
  id: given,
  children,
}: {
  label: string
  hint?: React.ReactNode
  error?: string | null
  /** A fixed id, e.g. to focus the field; the label and input always share it. */
  id?: string
  children: (props: { id: string; 'aria-invalid'?: true; 'aria-describedby'?: string }) => React.ReactNode
}) {
  const auto = useId()
  const id = given ?? auto
  const describedBy = [hint ? `${id}-hint` : '', error ? `${id}-error` : ''].filter(Boolean).join(' ') || undefined
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      {children({ id, 'aria-invalid': error ? true : undefined, 'aria-describedby': describedBy })}
      {hint && (
        <p className="hint" id={`${id}-hint`}>
          {hint}
        </p>
      )}
      {error && (
        <p className="field-error" id={`${id}-error`}>
          {error}
        </p>
      )}
    </div>
  )
}

export function Problem({ message }: { message: string | null }) {
  if (!message) return null
  return (
    <p className="notice" role="alert">
      {message}
    </p>
  )
}

export function Tile({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="fact">
      <div className="k">{label}</div>
      <div className="v">{value}</div>
      {sub && <div className="sub">{sub}</div>}
    </div>
  )
}

export function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
