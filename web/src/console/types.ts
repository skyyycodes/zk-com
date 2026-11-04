import type { Check, Fund, Status } from '../types'

export interface Step {
  label: string
  state: 'pending' | 'running' | 'done' | 'failed'
  ms: number | null
  detail: string | null
}

export interface JobResult {
  accepted: boolean
  saved?: boolean
  round: number
  reason?: string | null
  claim: string
  /** The share link, or null if the registry hasn't returned one yet. */
  page_url: string | null
  status?: Status | null
}

export interface Job {
  id: number
  running: boolean
  steps: Step[]
  result: JobResult | null
  error: string | null
}

export interface ReserveNote {
  value_zat: number
  height: number
  spent_height: number | null
  cmx: string
}

export interface PendingRound {
  file: string
  round: number
  at_height: number
  threshold_zat: number
}

export interface Unconfigured {
  mode: 'fund'
  configured: false
  wallets: string[]
  devnet_here: boolean
}

export interface Configured {
  mode: 'fund'
  configured: true
  network: string
  devnet: boolean
  unit: string
  registry_url: string
  registry_id: string
  data_dir: string
  tip: number | null
  wallet: {
    name: string
    address: string
    balance_zat: number
    operating_zat: number
    reserve: ReserveNote | null
    notes: number
  }
  fund: { id: string; key: string; page_url: string | null; registry: Fund | null } | null
  registry_error: string | null
  pending: PendingRound[]
  job: Job
}

export type ConsoleState = Unconfigured | Configured | AuthorityState

/* ---- certificates: the fund's side and the authority's side ---- */

export type Role = 'auditor' | 'regulator' | 'compliance' | 'registry'

export interface Certificate {
  version: string
  id: string
  kind: 'registry' | 'authority'
  registry: string
  issuer_name: string
  issuer_role: Role
  issuer_key: string
  fund_id: string
  fund_name: string
  fund_key: string
  round: number
  threshold_zat: number
  since_height: number
  at_height: number
  at_block_hash: string
  statement_hash: string
  checks_passed: number
  checks_total: number
  note: string
  issued_at: string
  signature: string
}

export interface CertRequest {
  id: string
  fund_id: string
  authority_key: string
  round: number
  note: string
  requested_at: string
  status: 'pending' | 'issued' | 'declined'
  reason: string | null
  decided_at: string | null
  certificate: Certificate | null
}

export interface Authority {
  key: string
  name: string
  role: Role
  approved_at: string
}

export interface Compliance {
  fund_id: string
  fund_name: string | null
  unit: string
  authorities: Authority[]
  rounds: number[]
  registry_certificates: Certificate[]
  requests: { request: CertRequest; authority_name: string | null; authority_role: Role | null }[]
}

export interface AuthorityState {
  mode: 'authority'
  configured: boolean
  name?: string
  role?: Role
  key?: string
  approved?: boolean
  network?: string
  registry_url?: string
  registry_id?: string
  data_dir?: string
  tip?: number | null
  registry_error?: string | null
}

export interface InboxAttestation {
  registry: string
  fund_id: string
  round: number
  threshold_zat: number
  since_height: number
  at_height: number
  at_block_hash: string
}

export interface InboxItem {
  request: CertRequest
  fund_name: string | null
  fund_description: string | null
  obligations_zat: number | null
  link: string | null
  attestation: InboxAttestation | null
}

export interface Inbox {
  authority: Authority
  requests: InboxItem[]
}

export interface CheckResult {
  verdict: { ok: boolean; checks: Check[] }
  ms: number
}
