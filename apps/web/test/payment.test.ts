import {
  Account,
  BASE_FEE,
  Contract,
  Keypair,
  Networks,
  Operation,
  Transaction,
  TransactionBuilder,
  nativeToScVal,
  xdr,
} from '@stellar/stellar-sdk'
import {
  STELLAR_HORIZON_URL,
  STELLAR_NETWORK,
  USDC_TESTNET_ISSUER,
  X402_HEADERS,
  type X402PaymentRequiredHeader,
  type X402PaymentSignatureHeader,
} from '@paperpay/shared'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { decodeBase64Json, encodeBase64Json } from '@/lib/base64'
import { PaywallError } from '@/lib/errors'
import { apiBackend } from '@/services/apiBackend'

// Freighter y el pago clásico, simulados: la prueba cubre lo que hace el sitio alrededor de la firma.
const signTxXdr = vi.hoisted(() => vi.fn(async (unsigned: string) => `SIGNED:${unsigned}`))
const buildPaymentXdr = vi.hoisted(() => vi.fn(async () => 'CLASSIC_XDR'))
vi.mock('@/services/wallet', () => ({ isFreighter: () => true, signTxXdr }))
vi.mock('@/services/stellarPayment', () => ({ buildPaymentXdr }))

const CONTRACT = 'CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5'
const PUBLISHER = 'GA3MOPOJ374G4APKR4RE7VPXF5CEQKER3FA3LZ2XSAARIRSZEF6QXUZI'
const TREASURY = 'GAB2NXBPZMJEPZPXMAAEZDJA7SB6ZQRHTT2KF4Z7LMOEAJKSP4JBCWRV'
const PRICE = '5000000'
const TX_HASH = 'a'.repeat(64)

const reader = Keypair.random().publicKey()
let paperSeq = 0
const nextPaper = () => `paper-${++paperSeq}` // el sitio guarda el 402 por artículo: uno nuevo por prueba

interface PayCall {
  contract?: string
  fn?: string
  payer?: string
  paperId: string
  publisher?: string
  amount?: string
  auth?: xdr.SorobanAuthorizationEntry[]
}

function payOp(call: PayCall) {
  return new Contract(call.contract ?? CONTRACT).call(
    call.fn ?? 'pay',
    nativeToScVal(call.payer ?? reader, { type: 'address' }),
    nativeToScVal(call.paperId, { type: 'string' }),
    nativeToScVal(call.publisher ?? PUBLISHER, { type: 'address' }),
    nativeToScVal(BigInt(call.amount ?? PRICE), { type: 'i128' }),
  )
}

/** El sobre que devolvería /prepare (sin simular: la prueba no habla con la red). */
function payXdr(call: PayCall): string {
  let op = payOp(call)
  if (call.auth) {
    const func = (txOf(op, call.payer).operations[0] as Operation.InvokeHostFunction).func
    op = Operation.invokeHostFunction({ func, auth: call.auth })
  }
  return txOf(op, call.payer).toXDR()
}

function txOf(op: xdr.Operation, source = reader): Transaction {
  return new TransactionBuilder(new Account(source, '1'), { fee: BASE_FEE, networkPassphrase: Networks.TESTNET })
    .addOperation(op)
    .setTimeout(300)
    .build()
}

function authFor(call: PayCall): xdr.SorobanAuthorizationEntry {
  const op = txOf(payOp(call)).operations[0] as Operation.InvokeHostFunction
  if (op.func.type !== 'hostFunctionTypeInvokeContract') throw new Error('no es invokeContract')
  return new xdr.SorobanAuthorizationEntry({
    credentials: xdr.SorobanCredentials.sorobanCredentialsSourceAccount(),
    rootInvocation: new xdr.SorobanAuthorizedInvocation({
      function: xdr.SorobanAuthorizedFunction.sorobanAuthorizedFunctionTypeContractFn(op.func.invokeContract),
      subInvocations: [],
    }),
  })
}

function requirement(id: string, splitter: boolean): X402PaymentRequiredHeader {
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
          ...(splitter
            ? { splitter: { contractId: CONTRACT, function: 'pay' as const, publisher: PUBLISHER, prepareUrl: `/api/papers/${id}/prepare` } }
            : {}),
        },
      },
    ],
  }
}

const json = (status: number, body: unknown, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', ...headers } })

const ACCESS_TOKEN = `h.${btoa(JSON.stringify({ exp: 1_900_000_000 }))}.s`

interface FakeApi {
  splitter?: boolean
  /** Saldos de la wallet en Horizon; 'missing' = la cuenta no existe. */
  balances?: { asset_code?: string; asset_issuer?: string; balance: string }[] | 'missing'
  /** XDR que devuelve /prepare; por defecto, el pago exacto del 402. */
  prepared?: (payer: string, paperId: string) => string
  /** Respuestas sucesivas al GET con PAYMENT-SIGNATURE ('throw' = sin respuesta). */
  settle?: (number | 'throw')[]
}

function installApi(api: FakeApi) {
  const calls = { prepare: [] as { payer: string }[], settle: [] as string[] }
  const settle = [...(api.settle ?? [200])]
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string, init: RequestInit = {}) => {
      const url = String(input)
      if (url.startsWith(`${STELLAR_HORIZON_URL}/accounts/`)) {
        if (api.balances === 'missing') return json(404, { status: 404 })
        return json(200, { balances: api.balances ?? [{ asset_code: 'USDC', asset_issuer: USDC_TESTNET_ISSUER, balance: '1.0000000' }] })
      }
      const match = url.match(/^\/api\/papers\/([^/]+)(\/prepare)?$/)
      if (!match) throw new Error(`URL inesperada: ${url}`)
      const id = decodeURIComponent(match[1])

      if (match[2]) {
        const body = JSON.parse(String(init.body)) as { payer: string }
        calls.prepare.push(body)
        const transactionXdr = api.prepared?.(body.payer, id) ?? payXdr({ payer: body.payer, paperId: id })
        return json(200, { transactionXdr, claim: `comprobante-${id}`, networkPassphrase: Networks.TESTNET, terms: { contractId: CONTRACT, publisher: PUBLISHER, amount: PRICE } })
      }

      const signature = new Headers(init.headers).get(X402_HEADERS.PAYMENT_SIGNATURE)
      if (!signature) {
        return json(402, { preview: { id } }, { [X402_HEADERS.PAYMENT_REQUIRED]: encodeBase64Json(requirement(id, api.splitter ?? true)) })
      }
      calls.settle.push(signature)
      const next = settle.shift() ?? 200
      if (next === 'throw') throw new TypeError('fetch failed')
      if (next === 200) return json(200, { paper: { id, fullContentMarkdown: '# completo' }, accessToken: ACCESS_TOKEN, txHash: TX_HASH })
      return json(next, { error: 'X', message: `respuesta ${next}` })
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

const signed = (header: string) => decodeBase64Json<X402PaymentSignatureHeader>(header)

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
  buildPaymentXdr.mockClear()
  vi.stubGlobal('window', { localStorage: memoryStorage(), sessionStorage: memoryStorage(), location: { search: '' } })
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('pago por el contrato de reparto', () => {
  it('pide /prepare, verifica el sobre, firma ese XDR y guarda el acceso', async () => {
    const calls = installApi({})
    const paper = nextPaper()
    const onSigned = vi.fn()

    const result = await apiBackend.payForPaper(paper, reader, onSigned)

    expect(calls.prepare).toEqual([{ payer: reader }])
    const prepared = payXdr({ paperId: paper })
    expect(signTxXdr).toHaveBeenCalledWith(prepared, reader)
    expect(buildPaymentXdr).not.toHaveBeenCalled()
    expect(onSigned).toHaveBeenCalledOnce()
    expect(signed(calls.settle[0])).toMatchObject({ signerPublicKey: reader, signature: `SIGNED:${prepared}`, claim: `comprobante-${paper}` })
    expect(result.receipt).toMatchObject({ txHash: TX_HASH, amount: 0.5 })
    expect(window.localStorage.getItem(`paperpay:access:${paper}`)).toContain(ACCESS_TOKEN)
  })

  it('sin extra.splitter en el 402 sigue con el pago clásico', async () => {
    const calls = installApi({ splitter: false })
    await apiBackend.payForPaper(nextPaper(), reader, () => {})
    expect(calls.prepare).toEqual([])
    expect(buildPaymentXdr).toHaveBeenCalledWith(reader, { payTo: TREASURY, amountStroops: PRICE })
    expect(signed(calls.settle[0]).signature).toBe('SIGNED:CLASSIC_XDR')
    expect(signed(calls.settle[0])).not.toHaveProperty('claim')
  })

  it('con ?memo= en la URL paga por la transferencia clásica con ese memo (Soroban no admite memo)', async () => {
    window.location.search = '?wallet=freighter&memo=PAPERPAY-PRUEBA'
    const calls = installApi({ splitter: true })
    await apiBackend.payForPaper(nextPaper(), reader, () => {})
    expect(calls.prepare).toEqual([])
    expect(buildPaymentXdr).toHaveBeenCalledWith(reader, { payTo: TREASURY, amountStroops: PRICE, memo: 'PAPERPAY-PRUEBA' })
    expect(signed(calls.settle[0]).signature).toBe('SIGNED:CLASSIC_XDR')
  })

  it('un memo de más de 28 bytes se rechaza antes de abrir la wallet', async () => {
    window.location.search = `?memo=${encodeURIComponent('é'.repeat(15))}`
    const calls = installApi({ splitter: true })
    expect((await failure(apiBackend.payForPaper(nextPaper(), reader, () => {}))).message).toMatch(/28 bytes/)
    expect(buildPaymentXdr).not.toHaveBeenCalled()
    expect(signTxXdr).not.toHaveBeenCalled()
    expect(calls.settle).toEqual([])
  })

  it.each<[string, Partial<PayCall>]>([
    ['otro monto', { amount: '50000000' }],
    ['otra función', { fn: 'gift' }],
    ['otro contrato', { contract: 'CDOUF6BBLKAXPT6EJAF2SOAUMKX3RASZFNHD7ORMIQZUTH4G6V66BPOZ' }],
    ['otra editorial', { publisher: TREASURY }],
    ['otro artículo', { paperId: 'otro-articulo' }],
    ['otra wallet', { payer: Keypair.random().publicKey() }],
  ])('no abre Freighter si el sobre preparado trae %s', async (_, change) => {
    const calls = installApi({ prepared: (payer, paperId) => payXdr({ payer, paperId, ...change }) })
    const err = await failure(apiBackend.payForPaper(nextPaper(), reader, () => {}))
    expect(err.code).toBe('PAYMENT_FAILED')
    expect(err.message).toMatch(/no coincide con el anunciado/)
    expect(signTxXdr).not.toHaveBeenCalled()
    expect(calls.settle).toEqual([])
  })

  it('no abre Freighter si el sobre autoriza otra llamada', async () => {
    installApi({
      prepared: (payer, paperId) => payXdr({ payer, paperId, auth: [authFor({ payer, paperId, amount: '50000000' })] }),
    })
    const err = await failure(apiBackend.payForPaper(nextPaper(), reader, () => {}))
    expect(err.message).toMatch(/autorización/)
    expect(signTxXdr).not.toHaveBeenCalled()
  })

  it('acepta el sobre simulado con la autorización del mismo pago', async () => {
    installApi({ prepared: (payer, paperId) => payXdr({ payer, paperId, auth: [authFor({ payer, paperId })] }) })
    await expect(apiBackend.payForPaper(nextPaper(), reader, () => {})).resolves.toMatchObject({ accessToken: ACCESS_TOKEN })
  })

  it.each<[string, FakeApi['balances'], string]>([
    ['la cuenta no existe', 'missing', 'INSUFFICIENT_FUNDS'],
    ['no hay trustline de USDC', [{ balance: '100.0000000' }], 'NO_TRUSTLINE'],
    ['el USDC no alcanza', [{ asset_code: 'USDC', asset_issuer: USDC_TESTNET_ISSUER, balance: '0.4999999' }], 'INSUFFICIENT_FUNDS'],
  ])('si %s avisa antes de preparar el pago', async (_, balances, code) => {
    const calls = installApi({ balances })
    const err = await failure(apiBackend.payForPaper(nextPaper(), reader, () => {}))
    expect(err.code).toBe(code)
    expect(calls.prepare).toEqual([])
  })
})

describe('pago firmado sin confirmar', () => {
  it.each<[string, number | 'throw']>([
    ['504 (la red no confirmó a tiempo)', 504],
    ['sin respuesta de la API', 'throw'],
  ])('%s: el reintento reenvía el mismo sobre, sin firmar otro', async (_, first) => {
    const calls = installApi({ settle: [first, 200] })
    const paper = nextPaper()

    const err = await failure(apiBackend.payForPaper(paper, reader, () => {}))
    expect(err.code).toBe('PAYMENT_PENDING')

    await expect(apiBackend.payForPaper(paper, reader, () => {})).resolves.toMatchObject({ receipt: { txHash: TX_HASH } })
    expect(signTxXdr).toHaveBeenCalledOnce()
    expect(calls.prepare).toHaveLength(1)
    expect(calls.settle).toHaveLength(2)
    expect(calls.settle[1]).toBe(calls.settle[0])
    // El reenvío lleva el comprobante de /prepare: la API da el acceso aunque pasen más de 5 min.
    expect(signed(calls.settle[1])).toMatchObject({ claim: `comprobante-${paper}` })
    expect(window.sessionStorage.getItem(`paperpay:pending:${paper}`)).toBeNull()
  })

  it('si la red rechaza el sobre pendiente, lo olvida y el siguiente intento firma uno nuevo', async () => {
    const calls = installApi({ settle: [504, 402, 200] })
    const paper = nextPaper()
    expect((await failure(apiBackend.payForPaper(paper, reader, () => {}))).code).toBe('PAYMENT_PENDING')
    expect((await failure(apiBackend.payForPaper(paper, reader, () => {}))).code).toBe('PAYMENT_FAILED')
    await apiBackend.payForPaper(paper, reader, () => {})
    expect(signTxXdr).toHaveBeenCalledTimes(2)
    expect(calls.settle[1]).toBe(calls.settle[0])
  })

  it('un pendiente de otra wallet no se reenvía', async () => {
    const calls = installApi({ settle: [504, 200] })
    const paper = nextPaper()
    await failure(apiBackend.payForPaper(paper, reader, () => {}))
    const other = Keypair.random().publicKey()
    await apiBackend.payForPaper(paper, other, () => {})
    expect(calls.prepare.map((p) => p.payer)).toEqual([reader, other])
    expect(signed(calls.settle[1]).signerPublicKey).toBe(other)
  })

  it('503: reenvía el mismo sobre dos veces más antes de rendirse', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout'] })
    const calls = installApi({ splitter: false, settle: [503, 503, 200] })
    const done = apiBackend.payForPaper(nextPaper(), reader, () => {})
    for (let i = 0; i < 3; i++) await vi.advanceTimersByTimeAsync(3_000)
    await expect(done).resolves.toMatchObject({ receipt: { txHash: TX_HASH } })
    expect(calls.settle).toHaveLength(3)
    expect(new Set(calls.settle).size).toBe(1)
    expect(signTxXdr).toHaveBeenCalledOnce()
  })

  it('503 persistente queda pendiente', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout'] })
    const calls = installApi({ splitter: false, settle: [503, 503, 503] })
    const done = failure(apiBackend.payForPaper(nextPaper(), reader, () => {}))
    for (let i = 0; i < 3; i++) await vi.advanceTimersByTimeAsync(3_000)
    expect((await done).code).toBe('PAYMENT_PENDING')
    expect(calls.settle).toHaveLength(3)
  })
})
