import type { Status } from '../types'

const LABEL: Record<Status, string> = {
  green: 'Reserve proven and covers obligations',
  amber: 'Reserve proven, with a caveat',
  red: 'Proof failed',
  none: 'No proof yet',
}

export function StatusDot({ status }: { status: Status }) {
  return <span className={`dot ${status}`} role="img" aria-label={LABEL[status]} />
}

export function StatusLight({ status }: { status: Status }) {
  return <span className={`light ${status}`} role="img" aria-label={LABEL[status]} />
}
