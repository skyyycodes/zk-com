// Shapes returned by the registry's JSON API (src/registry.rs).
// Amounts are zatoshi; 21M ZEC fits exactly in a JS number.

export interface Check {
  name: string
  ok: boolean
  detail: string
}

export interface Verdict {
  ok: boolean
  fund_id: string
  threshold_zat: number
  since_height: number
  at_height: number
  tip: number
  checks: Check[]
}

export interface Obligation {
  label: string
  zat: number
}

export interface LatestAttestation {
  round: number
  threshold_zat: number
  since_height: number
  at_height: number
  at_block_hash: string
  registry_nullifier: string | null
  registry_nullifiers: string[]
  proof_bytes: number
  prove_ms: number
  message: string
  received_at: string
  verdict: Verdict
  blocks_behind_tip: number
}

export type Status = 'green' | 'amber' | 'red' | 'none'

export interface Fund {
  id: string
  /** ZEC, or TAZ on testnet. */
  unit: string
  /** The fund's public key, bound at first registration; signs its profile and rounds. */
  key: string
  name: string
  description: string
  obligations: Obligation[]
  obligations_zat: number
  min_held_blocks: number
  /** The fund promises a new proof at least this often (0: no promise). */
  max_proof_age_blocks: number
  zec_usd: number | null
  price_time: string | null
  status: Status
  status_text: string
  latest: LatestAttestation | null
  rounds: number
  /** Set when the fund's most recent submission was refused. */
  last_refusal: string | null
  /** Accepted rounds and refusals, oldest first. */
  past: PastItem[]
}

export interface PastItem {
  /** Unix seconds. */
  time: string
  accepted: boolean
  round: number | null
  at_height: number | null
  light: Status
  text: string
}

/** Chain facts only: the registry lists no funds. A fund is reached by its share link. */
export interface RegistryState {
  registry: string
  network: string
  pool: string
  unit: string
  tip: number
  tip_hash: string
}

export interface HistoryEntry {
  height: number
  direction: 'in' | 'out'
  value_zat: number
  counterparty: string
  memo: string
}

/** Demo-only export of what the fund's viewing key would reveal. */
export interface ViewingKeyExport {
  note: string
  balance_zat: number
  history: HistoryEntry[]
}
