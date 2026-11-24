import { DEFAULT_PAPER_PRICE_USDC, STELLAR_HORIZON_URL, USDC_TESTNET_ISSUER, type HealthCheckResponse } from '@paperpay/shared'
import { stroopsToUsdc, toStroops } from '@/lib/amounts'
import { API_URL } from '@/lib/api'
import { contractAddressParam, readScSymbol, readScVecLength } from '@/lib/soroban'

/*
 * Lecturas pagadas, leídas de la cuenta de la tesorería en Horizon (público). Hay dos tipos:
 *  - Por contrato de reparto (el que anuncia /api/health): la llamada pay/gift/pay_batch reparte en
 *    la misma transacción, así que el 98/2 es real y se lee de los movimientos de USDC. pay y gift
 *    pagan 1 artículo; pay_batch, uno por elemento.
 *  - Transferencia directa de USDC a la tesorería (pago clásico): el 100% llega a la tesorería y el
 *    98/2 es una estimación. El ledger no dice qué artículo se leyó ni si vino de PaperPay.
 */

const CHART_DAYS = 14
const MAX_PAYMENTS = 200 // suficiente para la demo
const READS_PER_CALL: Record<string, 'one' | 'batch'> = { pay: 'one', gift: 'one', pay_batch: 'batch' }

export type ReadKind = 'contract' | 'direct'

export interface EditorialTx {
  hash: string
  /** USDC pagados en la transacción (todas sus lecturas). */
  amount: number
  /** Cuenta que pagó (en `gift`, quien regala). */
  from: string
  date: string
  kind: ReadKind
  /** Artículos pagados en la transacción. */
  reads: number
}

export interface EditorialStats {
  reads: number
  /** Lecturas que pasaron por el contrato de reparto (98/2 real). */
  contractReads: number
  totalUsdc: number
  publisherUsdc: number
  feeUsdc: number
  perDay: { label: string; count: number }[]
  recent: EditorialTx[]
  /** Cuenta de la tesorería, para enlazarla en el explorador. */
  treasury: string
  /** Horizon devolvió el máximo pedido: puede haber más pagos que los contados. */
  truncated: boolean
}

interface HorizonBalanceChange {
  asset_code?: string
  asset_issuer?: string
  type: string
  from: string
  to: string
  amount: string
}

export interface HorizonRecord {
  type: string
  transaction_successful?: boolean
  transaction_hash: string
  created_at: string
  source_account?: string
  // payment
  asset_code?: string
  asset_issuer?: string
  from?: string
  to?: string
  amount?: string
  // invoke_host_function
  parameters?: { value: string; type: string }[]
  asset_balance_changes?: HorizonBalanceChange[]
}

export interface ReadEntry extends EditorialTx {
  publisherStroops: bigint
  feeStroops: bigint
}

const isUsdc = (code?: string, issuer?: string) => code === 'USDC' && issuer === USDC_TESTNET_ISSUER

function directRead(record: HorizonRecord, treasury: string): ReadEntry | null {
  if (record.type !== 'payment' || !isUsdc(record.asset_code, record.asset_issuer) || record.to !== treasury) return null
  const total = toStroops(record.amount ?? '0')
  const publisher = (total * 98n) / 100n // estimado: todo llegó a la tesorería
  return {
    hash: record.transaction_hash,
    amount: stroopsToUsdc(total),
    from: record.from ?? '',
    date: record.created_at,
    kind: 'direct',
    reads: 1,
    publisherStroops: publisher,
    feeStroops: total - publisher,
  }
}

function contractRead(record: HorizonRecord, treasury: string, contractParam: string): ReadEntry | null {
  const params = record.parameters ?? []
  if (record.type !== 'invoke_host_function' || record.transaction_successful === false) return null
  if (params[0]?.value !== contractParam) return null

  const call = READS_PER_CALL[readScSymbol(params[1]?.value) ?? '']
  const reads = call === 'one' ? 1 : call === 'batch' ? readScVecLength(params[3]?.value) : 0
  if (reads === 0) return null

  const transfers = (record.asset_balance_changes ?? []).filter((c) => c.type === 'transfer' && isUsdc(c.asset_code, c.asset_issuer))
  let fee = 0n
  let publisher = 0n
  for (const t of transfers) {
    if (t.to === treasury) fee += toStroops(t.amount)
    else publisher += toStroops(t.amount)
  }
  if (fee + publisher === 0n) return null
  return {
    hash: record.transaction_hash,
    amount: stroopsToUsdc(fee + publisher),
    from: transfers[0]?.from ?? record.source_account ?? '',
    date: record.created_at,
    kind: 'contract',
    reads,
    publisherStroops: publisher,
    feeStroops: fee,
  }
}

/** Lecturas pagadas en los registros de `/accounts/{tesorería}/payments` (más recientes primero). */
export function readsFromRecords(
  records: HorizonRecord[],
  opts: { treasury: string; splitterContractId: string | null },
): ReadEntry[] {
  const contractParam = opts.splitterContractId ? contractAddressParam(opts.splitterContractId) : null
  const entries: ReadEntry[] = []
  for (const record of records) {
    const entry = directRead(record, opts.treasury) ?? (contractParam ? contractRead(record, opts.treasury, contractParam) : null)
    if (entry) entries.push(entry)
  }
  return entries
}

export function summarizeReads(entries: ReadEntry[], treasury: string, truncated: boolean, now = new Date()): EditorialStats {
  const counts = new Map<string, number>()
  let publisher = 0n
  let fee = 0n
  for (const e of entries) {
    const day = e.date.slice(0, 10)
    counts.set(day, (counts.get(day) ?? 0) + e.reads)
    publisher += e.publisherStroops
    fee += e.feeStroops
  }

  const perDay = Array.from({ length: CHART_DAYS }, (_, i) => {
    const d = new Date(now)
    d.setDate(d.getDate() - (CHART_DAYS - 1 - i))
    return {
      label: new Intl.DateTimeFormat('es-MX', { day: 'numeric', month: 'short' }).format(d),
      count: counts.get(d.toISOString().slice(0, 10)) ?? 0,
    }
  })

  return {
    reads: entries.reduce((sum, e) => sum + e.reads, 0),
    contractReads: entries.filter((e) => e.kind === 'contract').reduce((sum, e) => sum + e.reads, 0),
    totalUsdc: stroopsToUsdc(publisher + fee),
    publisherUsdc: stroopsToUsdc(publisher),
    feeUsdc: stroopsToUsdc(fee),
    perDay,
    recent: entries.slice(0, 8).map(({ publisherStroops: _p, feeStroops: _f, ...tx }) => tx),
    treasury,
    truncated,
  }
}

async function getHealth(): Promise<{ treasury: string; splitterContractId: string | null }> {
  const res = await fetch(`${API_URL}/api/health`, { signal: AbortSignal.timeout(10_000) })
  if (!res.ok) throw new Error('health')
  const body = (await res.json()) as Partial<HealthCheckResponse>
  if (!body.treasuryPublicKey) throw new Error('health sin tesorería')
  // Una API anterior al contrato no trae splitterContractId: solo se cuentan las directas.
  return { treasury: body.treasuryPublicKey, splitterContractId: body.splitterContractId ?? null }
}

export async function getEditorialStats(): Promise<EditorialStats> {
  const { treasury, splitterContractId } = await getHealth()

  const url = `${STELLAR_HORIZON_URL}/accounts/${treasury}/payments?order=desc&limit=${MAX_PAYMENTS}`
  const res = await fetch(url, { signal: AbortSignal.timeout(15_000) })
  if (!res.ok) throw new Error(`horizon ${res.status}`)
  const data = (await res.json()) as { _embedded: { records: HorizonRecord[] } }
  const records = data._embedded.records

  const entries = readsFromRecords(records, { treasury, splitterContractId })
  return summarizeReads(entries, treasury, records.length >= MAX_PAYMENTS)
}

// Una sola lectura por carga de página: el hero y la sección de números piden lo mismo.
// Si falla, se olvida para que un montaje posterior pueda reintentar.
let shared: Promise<EditorialStats> | null = null
export function getEditorialStatsShared(): Promise<EditorialStats> {
  if (!shared) {
    shared = getEditorialStats().catch((error: unknown) => {
      shared = null
      throw error
    })
  }
  return shared
}

export const READ_PRICE = DEFAULT_PAPER_PRICE_USDC
