import { Account, BASE_FEE, Contract, Keypair, Networks, TransactionBuilder, nativeToScVal } from '@stellar/stellar-sdk'
import {
  STELLAR_HORIZON_URL,
  STELLAR_NETWORK,
  USDC_TESTNET_ISSUER,
  X402_HEADERS,
  giftClaimMessage,
  type X402PaymentRequiredHeader,
  type X402PaymentSignatureHeader,
} from '@paperpay/shared'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { decodeBase64Json, encodeBase64Json } from '@/lib/base64'
import { PaywallError } from '@/lib/errors'
import { readAccess } from '@/services/accessStore'
import { canGift, claimGift, giftLink, listGifts, parseGiftFragment, sendGift } from '@/services/gift'

// Freighter simulado: la prueba cubre lo que hace el sitio alrededor de la firma.
const signTxXdr = vi.hoisted(() => vi.fn(async (unsigned: string) => `SIGNED:${unsigned}`))
const freighter = vi.hoisted(() => ({ on: true }))
vi.mock('@/services/wallet', () => ({ isFreighter: () => freighter.on, signTxXdr }))

const CONTRACT = 'CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5'
const PUBLISHER = 'GA3MOPOJ374G4APKR4RE7VPXF5CEQKER3FA3LZ2XSAARIRSZEF6QXUZI'
const TREASURY = 'GAB2NXBPZMJEPZPXMAAEZDJA7SB6ZQRHTT2KF4Z7LMOEAJKSP4JBCWRV'
const PRICE = '5000000'
const TX_HASH = 'b'.repeat(64)
const ACCESS_TOKEN = `h.${btoa(JSON.stringify({ exp: 1_900_000_000 }))}.s`

const payer = Keypair.random().publicKey()
let paperSeq = 0
const nextPaper = () => `regalo-${++paperSeq}` // el sitio guarda el 402 por artículo: uno nuevo por prueba

function giftXdr(from: string, reader: string, paperId: string, fn = 'gift'): string {
  const args = [
    nativeToScVal(from, { type: 'address' }),
    ...(fn === 'gift' ? [nativeToScVal(reader, { type: 'address' })] : []),
    nativeToScVal(paperId, { type: 'string' }),
    nativeToScVal(PUBLISHER, { type: 'address' }),
    nativeToScVal(BigInt(PRICE), { type: 'i128' }),
  ]
  return new TransactionBuilder(new Account(from, '1'), { fee: BASE_FEE, networkPassphrase: Networks.TESTNET })
    .addOperation(new Contract(CONTRACT).call(fn, ...args))
    .setTimeout(300)
    .build()
    .toXDR()
}

function requirement(id: string, gift: boolean): X402PaymentRequiredHeader {
  return {
    accepts: [
      {
        scheme: 'exact',
        network: STELLAR_NETWORK,
        asset: 'CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA',
        amount: PRICE,
        payTo: TREASURY,
        maxTimeoutSeconds: 60,
        extra: {
          paperId: id,
          title: 'Artículo de prueba',
          splitter: {
            contractId: CONTRACT,
            function: 'pay',
            publisher: PUBLISHER,
            prepareUrl: `/api/papers/${id}/prepare`,
            ...(gift && { giftUrl: `/api/papers/${id}/gift` }),
          },
        },
      },
    ],
  }
}

const json = (status: number, body: unknown, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', ...headers } })

interface FakeApi {
  gift?: boolean
  /** XDR que devuelve /prepare; por defecto, el gift exacto. */
  prepared?: (payer: string, reader: string, paperId: string) => string
  /** Respuestas sucesivas a POST giftUrl ('throw' = sin respuesta). */
  send?: (number | 'throw')[]
}

function installApi(api: FakeApi = {}) {
  const calls = { prepare: [] as { payer: string; reader?: string }[], send: [] as string[], claim: [] as Record<string, unknown>[] }
  const send = [...(api.send ?? [200])]
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string, init: RequestInit = {}) => {
      const url = String(input)
      if (url.startsWith(`${STELLAR_HORIZON_URL}/accounts/`)) {
        return json(200, { balances: [{ asset_code: 'USDC', asset_issuer: USDC_TESTNET_ISSUER, balance: '1.0000000' }] })
      }
      if (url === '/api/gifts/claim') {
        const body = JSON.parse(String(init.body)) as Record<string, unknown>
        calls.claim.push(body)
        return json(200, { paperId: 'articulo-regalado', paperTitle: 'Un regalo', accessToken: ACCESS_TOKEN, txHash: body.txHash, settledAt: '2026-11-24T23:30:00.000Z' })
      }
      const match = url.match(/^\/api\/papers\/([^/]+)(\/prepare|\/gift)?$/)
      if (!match) throw new Error(`URL inesperada: ${url}`)
      const id = decodeURIComponent(match[1])
      if (match[2] === '/prepare') {
        const body = JSON.parse(String(init.body)) as { payer: string; reader?: string }
        calls.prepare.push(body)
        const transactionXdr = api.prepared?.(body.payer, body.reader!, id) ?? giftXdr(body.payer, body.reader!, id)
        return json(200, { transactionXdr, networkPassphrase: Networks.TESTNET, terms: { contractId: CONTRACT, publisher: PUBLISHER, amount: PRICE } })
      }
      if (match[2] === '/gift') {
        const header = new Headers(init.headers).get(X402_HEADERS.PAYMENT_SIGNATURE)!
        calls.send.push(header)
        const next = send.shift() ?? 200
        if (next === 'throw') throw new TypeError('fetch failed')
        if (next !== 200) return json(next, { error: 'X', message: `respuesta ${next}` })
        const reader = calls.prepare.at(-1)!.reader
        return json(200, { txHash: TX_HASH, paperId: id, reader, settledAt: '2026-11-24T23:30:00.000Z' })
      }
      return json(402, { preview: { id } }, { [X402_HEADERS.PAYMENT_REQUIRED]: encodeBase64Json(requirement(id, api.gift ?? true)) })
    }),
  )
  return calls
}

function memoryStorage(): Storage {
  const data = new Map<string, string>()
  return {
    getItem: (k) => data.get(k) ?? null,
    setItem: (k, v) => void data.set(k, String(v)),
    removeItem: (k) => void data.delete(k),
    clear: () => data.clear(),
    key: (i) => [...data.keys()][i] ?? null,
    get length() {
      return data.size
    },
  }
}

async function failure(promise: Promise<unknown>): Promise<PaywallError> {
  const err = await promise.then(
    () => null,
    (e: unknown) => e,
  )
  expect(err).toBeInstanceOf(PaywallError)
  return err as PaywallError
}

beforeEach(() => {
  signTxXdr.mockClear()
  freighter.on = true
  vi.stubGlobal('window', { localStorage: memoryStorage(), sessionStorage: memoryStorage(), location: { search: '', origin: 'https://paperpay.test' } })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('regalar un artículo', () => {
  it('solo se ofrece si la API anuncia giftUrl y la wallet es Freighter', async () => {
    installApi({ gift: true })
    expect(await canGift(nextPaper())).toBe(true)
    installApi({ gift: false })
    expect(await canGift(nextPaper())).toBe(false)
    installApi({ gift: true })
    freighter.on = false
    expect(await canGift(nextPaper())).toBe(false)
  })

  it('crea una llave nueva, firma gift a su nombre y devuelve el enlace con el secreto', async () => {
    const calls = installApi()
    const id = nextPaper()
    const gift = await sendGift(id, payer)

    expect(calls.prepare).toHaveLength(1)
    expect(calls.prepare[0].payer).toBe(payer)
    expect(calls.prepare[0].reader).toBe(gift.reader)
    expect(Keypair.fromSecret(gift.secret).publicKey()).toBe(gift.reader)
    expect(gift).toMatchObject({ status: 'sent', txHash: TX_HASH, paperId: id })

    const header = decodeBase64Json<X402PaymentSignatureHeader>(calls.send[0])
    expect(header.signerPublicKey).toBe(payer)
    expect(header.signature).toMatch(/^SIGNED:/)

    const link = giftLink(gift)
    expect(link).toBe(`https://paperpay.test/regalo#${TX_HASH}.${gift.secret}`)
    expect(parseGiftFragment(new URL(link).hash)).toEqual({ txHash: TX_HASH, secret: gift.secret })
    // El enlace queda en el navegador de quien regala.
    expect(listGifts(id).map((g) => g.reader)).toEqual([gift.reader])
  })

  it('no abre Freighter si /prepare devuelve otra cosa (pay, u otro destinatario) y no guarda la llave', async () => {
    const id = nextPaper()
    installApi({ prepared: (from, _reader, paperId) => giftXdr(from, _reader, paperId, 'pay') })
    expect((await failure(sendGift(id, payer))).message).toMatch(/no coincide/)
    installApi({ prepared: (from, _reader, paperId) => giftXdr(from, Keypair.random().publicKey(), paperId) })
    expect((await failure(sendGift(id, payer))).message).toMatch(/no coincide/)
    expect(signTxXdr).not.toHaveBeenCalled()
    expect(listGifts(id)).toEqual([])
    // Ni siquiera como borrador: una llave sin pago no sirve para nada.
    expect(JSON.parse(window.localStorage.getItem('paperpay:gifts') ?? '[]')).toEqual([])
  })

  it('la llave queda guardada ANTES de abrir Freighter (si la pestaña se cierra después de pagar, no se pierde)', async () => {
    installApi()
    const id = nextPaper()
    let storedWhileSigning: { reader: string; secret: string }[] = []
    signTxXdr.mockImplementationOnce(async (unsigned: string) => {
      storedWhileSigning = JSON.parse(window.localStorage.getItem('paperpay:gifts') ?? '[]')
      return `SIGNED:${unsigned}`
    })
    const gift = await sendGift(id, payer)
    expect(storedWhileSigning).toMatchObject([{ reader: gift.reader, secret: gift.secret }])
  })

  it('si la red no confirma a tiempo, el siguiente intento reenvía el mismo sobre sin firmar otro', async () => {
    const calls = installApi({ send: [504, 200] })
    const id = nextPaper()
    expect((await failure(sendGift(id, payer))).code).toBe('PAYMENT_PENDING')
    expect(listGifts(id)).toMatchObject([{ status: 'pending' }])

    const gift = await sendGift(id, payer)
    expect(signTxXdr).toHaveBeenCalledOnce()
    expect(calls.prepare).toHaveLength(1)
    expect(calls.send[1]).toBe(calls.send[0])
    expect(gift.status).toBe('sent')
  })

  it('sin respuesta de la API, también queda pendiente (el secreto no se pierde)', async () => {
    installApi({ send: ['throw'] })
    const id = nextPaper()
    expect((await failure(sendGift(id, payer))).code).toBe('PAYMENT_PENDING')
    const [pending] = listGifts(id)
    expect(Keypair.fromSecret(pending.secret).publicKey()).toBe(pending.reader)
  })
})

describe('abrir un regalo', () => {
  it('rechaza fragmentos incompletos', () => {
    expect(parseGiftFragment('#abc')).toBeNull()
    expect(parseGiftFragment(`#${TX_HASH}`)).toBeNull()
    expect(parseGiftFragment(`#${TX_HASH}.SNOVALIDO`)).toBeNull()
  })

  it('firma el mensaje con la llave del enlace y guarda el acceso como una compra', async () => {
    const calls = installApi()
    const key = Keypair.random()
    const opened = await claimGift(TX_HASH, key.secret())
    expect(opened.paperId).toBe('articulo-regalado')

    const [body] = calls.claim
    expect(body.reader).toBe(key.publicKey())
    const message = Buffer.from(giftClaimMessage(TX_HASH, body.signedAt as number))
    expect(key.verify(message, Buffer.from(body.signature as string, 'base64'))).toBe(true)
    expect(Math.abs((body.signedAt as number) - Date.now() / 1000)).toBeLessThan(5)

    expect(readAccess('articulo-regalado')).toMatchObject({ accessToken: ACCESS_TOKEN, receipt: { txHash: TX_HASH, amount: 0.5 } })
  })

  it('un secreto inválido no llega a la API', async () => {
    const calls = installApi()
    expect((await failure(claimGift(TX_HASH, 'SNOVALIDO'))).message).toMatch(/incompleto/)
    expect(calls.claim).toEqual([])
  })
})
