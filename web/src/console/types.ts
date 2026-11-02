import type { Fund, Status } from '../types'

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
  page_url: string
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
  configured: false
  wallets: string[]
  devnet_here: boolean
}

export interface Configured {
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
  fund: { id: string; key: string; page_url: string; registry: Fund | null } | null
  registry_error: string | null
  pending: PendingRound[]
  job: Job
}

export type ConsoleState = Unconfigured | Configured
