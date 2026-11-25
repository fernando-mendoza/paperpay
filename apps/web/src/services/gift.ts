import {
  DEFAULT_PAPER_PRICE_STROOPS,
  X402_HEADERS,
  giftClaimMessage,
  type ApiErrorResponse,
  type GiftClaimResponse,
  type GiftSentResponse,
  type PaymentReceiptResponse,
  type X402PaymentSignatureHeader,
} from '@paperpay/shared'
import { API_URL } from '@/lib/api'
import { encodeBase64Json } from '@/lib/base64'
import { PaywallError } from '@/lib/errors'
import { getPaymentTerms, persistVerifiedAccess } from './apiBackend'
import { assertPreparedPayment, checkReaderFunds, prepareSplitterPayment } from './splitterPayment'
import { isFreighter, signTxXdr } from './wallet'

/*
 * Regalar un artículo (contrato de reparto, función `gift`). El navegador de quien regala crea una
 * llave nueva: su dirección es quien recibe el acceso en la red, y su secreto viaja en el fragmento
 * del enlace (`/regalo#<txHash>.<secreto>`), que el navegador no manda al servidor. Quien abre el
 * enlace firma con esa llave y la API le da el acceso, sin wallet. Quien tiene el enlace tiene el
 * regalo.
 */

export interface SentGift {
  paperId: string
  payer: string
  reader: string
  secret: string
  createdAt: string
  // signing: aún sin firmar · pending: firmado, sin confirmar · sent: confirmado · failed: la API lo rechazó
  status: 'signing' | 'pending' | 'sent' | 'failed'
  header?: string
  txHash?: string
}

const GIFTS_KEY = 'paperpay:gifts'
const SETTLE_TIMEOUT_MS = 45_000
const BUSY_RETRIES = 2
const BUSY_RETRY_MS = 3_000

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

// Los regalos se guardan en localStorage ANTES de firmar: si la pestaña se cierra después de pagar,
// el secreto (la única llave del regalo) no se pierde.
function readGifts(): SentGift[] {
  try {
    const raw = window.localStorage.getItem(GIFTS_KEY)
    return raw ? (JSON.parse(raw) as SentGift[]) : []
  } catch {
    return []
  }
}

function saveGift(gift: SentGift | null, reader: string): void {
  try {
    const others = readGifts().filter((g) => g.reader !== reader)
    window.localStorage.setItem(GIFTS_KEY, JSON.stringify(gift ? [...others, gift] : others))
  } catch {
    // Sin almacenamiento: el enlace solo se puede copiar en esta visita.
  }
}

/** Regalos de este navegador que ya se pagaron o siguen pendientes, del más nuevo al más viejo. */
export function listGifts(paperId: string): SentGift[] {
  return readGifts()
    .filter((g) => g.paperId === paperId && (g.status === 'sent' || g.status === 'pending'))
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
}

export function giftLink(gift: Pick<SentGift, 'txHash' | 'secret'>, origin = window.location.origin): string {
  return `${origin}/regalo#${gift.txHash}.${gift.secret}`
}

export function parseGiftFragment(hash: string): { txHash: string; secret: string } | null {
  const match = /^#?([0-9a-f]{64})\.(S[A-Z2-7]{55})$/i.exec(hash.trim())
  return match ? { txHash: match[1].toLowerCase(), secret: match[2].toUpperCase() } : null
}

/** ¿Esta API acepta regalos y la wallet es Freighter? (Pollar no paga por el contrato.) */
export async function canGift(paperId: string): Promise<boolean> {
  if (!isFreighter()) return false
  try {
    return Boolean((await getPaymentTerms(paperId)).extra?.splitter?.giftUrl)
  } catch {
    return false
  }
}

async function postGift(giftUrl: string, header: string): Promise<Response> {
  for (let attempt = 0; ; attempt++) {
    const res = await fetch(`${API_URL}${giftUrl}`, {
      method: 'POST',
      headers: { [X402_HEADERS.PAYMENT_SIGNATURE]: header },
      signal: AbortSignal.timeout(SETTLE_TIMEOUT_MS),
    })
    if (res.status !== 503 || attempt >= BUSY_RETRIES) return res
    await delay(BUSY_RETRY_MS)
  }
}

/**
 * Paga el regalo con la wallet de `payer` y devuelve el regalo confirmado (con su enlace). Si hay
 * un regalo de este artículo firmado y sin confirmar, reenvía ese sobre en vez de firmar otro.
 */
export async function sendGift(paperId: string, payer: string, onSigned: () => void = () => {}): Promise<SentGift> {
  const terms = await getPaymentTerms(paperId)
  const splitter = terms.extra?.splitter
  if (!splitter?.giftUrl?.startsWith('/api/')) throw new PaywallError('PAYMENT_FAILED', 'Esta API no acepta regalos')

  let gift = readGifts().find((g) => g.paperId === paperId && g.payer === payer && g.status === 'pending' && g.header)
  if (!gift) {
    const { Keypair } = await import('@stellar/stellar-sdk')
    const key = Keypair.random()
    const draft: SentGift = {
      paperId,
      payer,
      reader: key.publicKey(),
      secret: key.secret(),
      createdAt: new Date().toISOString(),
      status: 'signing',
    }
    saveGift(draft, draft.reader)
    try {
      await checkReaderFunds(payer, terms.amount)
      const prepared = await prepareSplitterPayment(splitter, payer, draft.reader)
      await assertPreparedPayment(prepared, {
        contractId: splitter.contractId,
        publisher: splitter.publisher,
        payer,
        paperId,
        amountStroops: terms.amount,
        reader: draft.reader,
      })
      const signature: X402PaymentSignatureHeader = {
        scheme: 'exact',
        network: terms.network,
        signerPublicKey: payer,
        signature: await signTxXdr(prepared.transactionXdr, payer),
      }
      gift = { ...draft, status: 'pending', header: encodeBase64Json(signature) }
      saveGift(gift, gift.reader)
    } catch (err) {
      saveGift(null, draft.reader) // nada se firmó: la llave no sirve para nada
      throw err
    }
  }
  onSigned()

  let res: Response
  try {
    res = await postGift(splitter.giftUrl, gift.header!)
  } catch {
    throw new PaywallError('PAYMENT_PENDING', 'Sin respuesta de la API') // se queda pending: se reenvía
  }
  const body = (await res.json().catch(() => ({}))) as Partial<GiftSentResponse & ApiErrorResponse>
  if (res.status === 503 || res.status === 504) throw new PaywallError('PAYMENT_PENDING', body.message)
  if (!res.ok || !body.txHash || body.reader !== gift.reader) {
    // Se conserva el secreto por si el pago sí llegó a la red; ya no se reenvía.
    saveGift({ ...gift, status: 'failed' }, gift.reader)
    throw new PaywallError('PAYMENT_FAILED', body.message ?? `No se pudo enviar el regalo (${res.status})`)
  }
  const sent: SentGift = { ...gift, status: 'sent', txHash: body.txHash, header: undefined }
  saveGift(sent, sent.reader)
  return sent
}

/** Recibo público del regalo (para mostrar el título antes de abrirlo). `null` si aún no existe. */
export async function fetchGiftReceipt(txHash: string): Promise<PaymentReceiptResponse | null> {
  const res = await fetch(`${API_URL}/api/receipts/${txHash}`, { signal: AbortSignal.timeout(10_000) })
  if (res.status === 404) return null
  if (!res.ok) throw new Error(`GET /api/receipts → ${res.status}`)
  return (await res.json()) as PaymentReceiptResponse
}

function toBase64(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes))
}

/** Firma con la llave del enlace y guarda el acceso de 24 h, igual que una compra. */
export async function claimGift(txHash: string, secret: string): Promise<GiftClaimResponse> {
  const { Keypair } = await import('@stellar/stellar-sdk')
  let key
  try {
    key = Keypair.fromSecret(secret)
  } catch {
    throw new PaywallError('PAYMENT_FAILED', 'El enlace de regalo está incompleto')
  }
  const signedAt = Math.floor(Date.now() / 1000)
  // sign() acepta y devuelve bytes; en el navegador no hay Buffer global.
  const message = new TextEncoder().encode(giftClaimMessage(txHash, signedAt))
  const signature = toBase64(new Uint8Array(key.sign(message as Parameters<typeof key.sign>[0])))

  let res: Response
  try {
    res = await fetch(`${API_URL}/api/gifts/claim`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ txHash, reader: key.publicKey(), signedAt, signature }),
      signal: AbortSignal.timeout(20_000),
    })
  } catch {
    throw new PaywallError('PAYMENT_FAILED', 'API no disponible')
  }
  const body = (await res.json().catch(() => ({}))) as Partial<GiftClaimResponse & ApiErrorResponse>
  if (!res.ok || !body.accessToken || !body.paperId) {
    throw new PaywallError('PAYMENT_FAILED', body.message ?? `No se pudo abrir el regalo (${res.status})`)
  }
  persistVerifiedAccess(body.paperId, { accessToken: body.accessToken, txHash: body.txHash ?? txHash }, DEFAULT_PAPER_PRICE_STROOPS)
  return body as GiftClaimResponse
}
