import type { Obligation } from '../types'
import type { ConsoleState, Job } from './types'

const KEY = 'zsolv-console-token'
let token: string | null = null

/** `zsolv console` prints a link whose #fragment carries this run's access token. Keep it for this tab only. */
export function takeToken(): string | null {
  const m = window.location.hash.match(/token=([0-9a-f]{64})/)
  if (m) {
    token = m[1]
    try {
      sessionStorage.setItem(KEY, token)
    } catch {
      // storage blocked: keep it in memory for this page
    }
    history.replaceState(null, '', `${window.location.pathname}#/`)
  } else if (!token) {
    try {
      token = sessionStorage.getItem(KEY)
    } catch {
      token = null
    }
  }
  return token
}

export class ConsoleError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message)
  }
}

async function call<T>(method: 'GET' | 'POST', path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api/console/${path}`, {
    method,
    headers: { 'X-Console-Token': token ?? '', ...(body === undefined ? {} : { 'Content-Type': 'application/json' }) },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const data = await res.json().catch(() => null)
  if (!res.ok) {
    const message = data && typeof data === 'object' && 'error' in data ? String(data.error) : `${res.status} ${res.statusText}`
    throw new ConsoleError(res.status, message)
  }
  return data as T
}

export interface SetupBody {
  mode: 'create' | 'import' | 'existing'
  network: 'devnet' | 'testnet'
  registry_url: string
  wallet?: string
  words?: string
  birthday?: number
}

export interface RegisterBody {
  id: string
  name: string
  description: string
  obligations: Obligation[]
  min_held_blocks: number
  max_proof_age_blocks: number
}

export interface MoveBody {
  amount: string
  label: string
  memo: string
}

export const consoleApi = {
  state: () => call<ConsoleState>('GET', 'state'),
  job: () => call<Job>('GET', 'job'),
  setup: (b: SetupBody) => call<{ ok: boolean; words: string | null; fund_id: string | null }>('POST', 'setup', b),
  register: (b: RegisterBody) => call<{ ok: boolean; id: string; page_url: string }>('POST', 'register', b),
  reserve: () => call<{ locked_zat: number; height: number }>('POST', 'reserve', {}),
  prove: () => call<{ started: number }>('POST', 'prove', {}),
  retry: () => call<{ results: { round: number; accepted: boolean; reason: string }[] }>('POST', 'retry', {}),
  receive: (b: MoveBody) => call<{ height: number }>('POST', 'receive', b),
  pay: (b: MoveBody) => call<{ height: number }>('POST', 'pay', b),
  mine: (blocks: number) => call<{ tip: number }>('POST', 'mine', { blocks }),
  sync: () => call<{ tip: number; notes: number }>('POST', 'sync', {}),
}
