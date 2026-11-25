import {
  ACCESS_TOKEN_EXPIRATION_HOURS,
  X402_HEADERS,
  type ApiErrorResponse,
  type JWTAccessTokenClaims,
  type X402PaymentRequiredHeader,
  type X402PaymentResponseHeader,
  type X402PaymentSignatureHeader,
} from '@paperpay/shared'
import { API_URL } from '@/lib/api'
import { decodeBase64Json, encodeBase64Json } from '@/lib/base64'
import { PaywallError } from '@/lib/errors'
import { memoFromUrl } from '@/lib/memo'
import type { PaperFull, PaperPreview, PaywallErrorCode, TxReceipt } from '@/lib/types'
import { clearAccess, isAccessValid, readAccess, writeAccess } from './accessStore'
import type { Backend, PaperResult } from './backend'
import { assertPreparedPayment, checkReaderFunds, prepareSplitterPayment } from './splitterPayment'
import { buildPaymentXdr } from './stellarPayment'
import { isFreighter, signTxXdr } from './wallet'

// Wallet simulada (NEXT_PUBLIC_WALLET != freighter): firma de relleno; el backend responde
// con un hash simulado. Con Freighter se construye y firma una transacción real.
const PLACEHOLDER_SIGNATURE = 'unsigned-demo-signature'

const STROOPS_PER_UNIT = 10_000_000

// La API espera hasta ~30 s la confirmación de un pago por contrato antes de responder 504.
const SETTLE_TIMEOUT_MS = 45_000
// 503 = la red no aceptó el envío por ahora: se reenvía el mismo sobre un par de veces.
const BUSY_RETRIES = 2
const BUSY_RETRY_MS = 3_000

// Condiciones de pago del último 402 por artículo, para no pedirlas dos veces.
const requirements = new Map<string, X402PaymentRequiredHeader>()

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

async function request(path: string, init: RequestInit = {}, timeoutMs = 10_000): Promise<Response> {
  try {
    return await fetch(`${API_URL}${path}`, { ...init, signal: AbortSignal.timeout(timeoutMs) })
  } catch {
    throw new PaywallError('PAYMENT_FAILED', 'API no disponible')
  }
}

function rememberRequirement(id: string, res: Response): void {
  const raw = res.headers.get(X402_HEADERS.PAYMENT_REQUIRED)
  if (raw) requirements.set(id, decodeBase64Json<X402PaymentRequiredHeader>(raw))
}

async function lockedResult(id: string, res: Response, expired: boolean): Promise<PaperResult> {
  rememberRequirement(id, res)
  const body = (await res.json()) as { preview: PaperPreview }
  return { status: 'locked', preview: body.preview, expired }
}

function tokenExpiry(accessToken: string): string {
  try {
    const claims = decodeBase64Json<JWTAccessTokenClaims>(accessToken.split('.')[1])
    if (claims.exp) return new Date(claims.exp * 1000).toISOString()
  } catch {
    // Token sin payload legible: usamos la vigencia estándar.
  }
  return new Date(Date.now() + ACCESS_TOKEN_EXPIRATION_HOURS * 3600 * 1000).toISOString()
}

function errorCodeFor(message = ''): PaywallErrorCode {
  if (/insufficient|saldo|fondos|balance/i.test(message)) return 'INSUFFICIENT_FUNDS'
  if (/trustline|l[ií]nea de confianza/i.test(message)) return 'NO_TRUSTLINE'
  return 'PAYMENT_FAILED'
}

// Pago firmado que quedó sin confirmar (504, 503 persistente o sin respuesta). El siguiente intento
// reenvía ESE sobre en vez de firmar otro: la red aplica un sobre una sola vez, así que no hay doble
// cobro. Vive en sessionStorage para sobrevivir a una recarga de la pestaña.
interface PendingPayment {
  header: string
  signer: string
}
const PENDING_PREFIX = 'paperpay:pending:'

function readPending(id: string, signer: string): PendingPayment | null {
  try {
    const raw = window.sessionStorage.getItem(PENDING_PREFIX + id)
    const pending = raw ? (JSON.parse(raw) as PendingPayment) : null
    return pending?.signer === signer ? pending : null
  } catch {
    return null
  }
}

function writePending(id: string, pending: PendingPayment | null): void {
  try {
    if (pending) window.sessionStorage.setItem(PENDING_PREFIX + id, JSON.stringify(pending))
    else window.sessionStorage.removeItem(PENDING_PREFIX + id)
  } catch {
    // Sin almacenamiento: el reintento firmará un pago nuevo.
  }
}

type Terms = X402PaymentRequiredHeader['accepts'][number]

// Devuelve el XDR firmado (o la firma de relleno en modo simulado). Abre Freighter si aplica. En el
// pago por contrato trae además el comprobante de /prepare: viaja en la cabecera y queda guardado con
// el pago pendiente, para que reenviar el sobre dé acceso aunque la confirmación tarde más de 5 min.
async function signPayment(id: string, address: string, terms: Terms): Promise<{ signature: string; claim?: string }> {
  if (!isFreighter()) {
    await delay(800)
    return { signature: PLACEHOLDER_SIGNATURE }
  }
  // `?memo=` en la URL del artículo: el pago lleva ese memo. Las transacciones de Soroban no admiten
  // memo, así que ese pago va por la transferencia clásica a la tesorería (sin el reparto 98/2 en la red).
  let memo: string | undefined
  try {
    memo = memoFromUrl(window.location.search)
  } catch (err) {
    throw new PaywallError('PAYMENT_FAILED', err instanceof Error ? err.message : 'Memo inválido')
  }
  const splitter = memo ? undefined : terms.extra?.splitter
  if (splitter) {
    // Contrato de reparto: la API arma y simula pay(...); aquí se verifica y se firma.
    await checkReaderFunds(address, terms.amount)
    const prepared = await prepareSplitterPayment(splitter, address)
    await assertPreparedPayment(prepared, {
      contractId: splitter.contractId,
      publisher: splitter.publisher,
      payer: address,
      paperId: id,
      amountStroops: terms.amount,
    })
    return { signature: await signTxXdr(prepared.transactionXdr, address), claim: prepared.claim }
  }
  // Pago clásico: transferencia de USDC a la tesorería.
  const unsignedXdr = await buildPaymentXdr(address, { payTo: terms.payTo, amountStroops: terms.amount, memo })
  return { signature: await signTxXdr(unsignedXdr, address) }
}

async function sendSignedPayment(path: string, header: string): Promise<Response> {
  for (let attempt = 0; ; attempt++) {
    const res = await request(path, { headers: { [X402_HEADERS.PAYMENT_SIGNATURE]: header } }, SETTLE_TIMEOUT_MS)
    if (res.status !== 503 || attempt >= BUSY_RETRIES) return res
    await delay(BUSY_RETRY_MS)
  }
}

async function paymentRequirement(id: string): Promise<X402PaymentRequiredHeader> {
  const cached = requirements.get(id)
  if (cached) return cached
  const res = await request(`/api/papers/${encodeURIComponent(id)}`)
  rememberRequirement(id, res)
  const fresh = requirements.get(id)
  if (!fresh) throw new PaywallError('PAYMENT_FAILED', 'El API no devolvió condiciones de pago')
  return fresh
}

export const apiBackend: Backend = {
  async getPapers() {
    const res = await request('/api/papers')
    if (!res.ok) throw new Error(`GET /api/papers → ${res.status}`)
    return (await res.json()) as PaperPreview[]
  },

  async getPaper(id) {
    const path = `/api/papers/${encodeURIComponent(id)}`
    const access = readAccess(id)
    const valid = access !== null && isAccessValid(access)

    if (access && valid) {
      const res = await request(path, { headers: { Authorization: `Bearer ${access.accessToken}` } })
      if (res.ok) {
        const body = (await res.json()) as { paper: PaperFull }
        return { status: 'unlocked', paper: body.paper, receipt: access.receipt }
      }
      clearAccess(id) // el API rechazó el token (p. ej. se reinició con otro secreto)
      if (res.status === 402) return lockedResult(id, res, false)
    }
    if (access && !valid) clearAccess(id)

    const res = await request(path)
    if (res.status === 404) throw new Error('PAPER_NOT_FOUND')
    if (res.status !== 402) throw new Error(`GET ${path} → ${res.status}`)
    return lockedResult(id, res, access !== null && !valid)
  },

  async payForPaper(id, address, onSigned) {
    const terms = (await paymentRequirement(id)).accepts[0]

    let pending = readPending(id, address)
    if (!pending) {
      const { signature: signed, claim } = await signPayment(id, address, terms)
      const signature: X402PaymentSignatureHeader = {
        scheme: 'exact',
        network: terms.network,
        signerPublicKey: address,
        signature: signed,
        ...(claim && { claim }),
      }
      pending = { header: encodeBase64Json(signature), signer: address }
    }
    onSigned()

    let res: Response
    try {
      res = await sendSignedPayment(`/api/papers/${encodeURIComponent(id)}`, pending.header)
    } catch {
      // Sin respuesta: el pago firmado pudo llegar a la red. Se reenvía, no se firma otro.
      writePending(id, pending)
      throw new PaywallError('PAYMENT_PENDING', 'Sin respuesta de la API')
    }

    if (res.status === 503 || res.status === 504) {
      writePending(id, pending)
      const body = (await res.json().catch(() => ({}))) as Partial<ApiErrorResponse>
      throw new PaywallError('PAYMENT_PENDING', body.message)
    }
    writePending(id, null)

    if (!res.ok) {
      const body = (await res.json().catch(() => ({}))) as Partial<ApiErrorResponse>
      throw new PaywallError(errorCodeFor(body.message), body.message)
    }

    const body = (await res.json()) as { paper: PaperFull; accessToken: string; txHash: string }
    const rawResponse = res.headers.get(X402_HEADERS.PAYMENT_RESPONSE)
    const settlement = rawResponse ? decodeBase64Json<X402PaymentResponseHeader>(rawResponse) : null

    const receipt: TxReceipt = {
      txHash: settlement?.txHash ?? body.txHash,
      amount: Number(terms.amount) / STROOPS_PER_UNIT,
      date: settlement?.settledAt || new Date().toISOString(),
      validUntil: tokenExpiry(body.accessToken),
    }
    writeAccess(id, { accessToken: body.accessToken, receipt })
    requirements.delete(id)
    return { paper: body.paper, accessToken: body.accessToken, receipt }
  },
}

// Condiciones de pago (payTo, monto, red) del 402 del backend. Lo usa el checkout de Pollar (v2)
// para construir la transacción con runTx.
export async function getPaymentTerms(id: string): Promise<X402PaymentRequiredHeader['accepts'][number]> {
  return (await paymentRequirement(id)).accepts[0]
}

// Tras verificar un pago por hash (Pollar v2), guarda el acceso y arma el recibo, igual que
// hace payForPaper con Freighter. Así el estado de la app queda idéntico entre v1 y v2.
export function persistVerifiedAccess(
  id: string,
  result: { accessToken: string; txHash: string },
  amountStroops: string,
): TxReceipt {
  const receipt: TxReceipt = {
    txHash: result.txHash,
    amount: Number(amountStroops) / STROOPS_PER_UNIT,
    date: new Date().toISOString(),
    validUntil: tokenExpiry(result.accessToken),
  }
  writeAccess(id, { accessToken: result.accessToken, receipt })
  requirements.delete(id)
  return receipt
}
