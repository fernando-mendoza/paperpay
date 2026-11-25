/**
 * E2E en testnet del regalo por enlace. SÍ mueve fondos de prueba: crea un lector desechable
 * (Friendbot + 1 USDC del DEX de testnet) que paga 0.50 USDC para que otra persona lea.
 *
 * Necesita una API con el contrato activo (ver apps/api/README.md):
 *   PAPERPAY_E2E_API_URL=http://127.0.0.1:4011 pnpm --filter @paperpay/web test:e2e
 *
 * Con PAPERPAY_E2E_GIFT_ORIGIN=http://localhost:3000 imprime además el enlace, para abrirlo en un navegador.
 *
 * Corre el código real del sitio (services/gift + apiBackend). Solo Freighter está simulado: la firma
 * la hace la llave de quien regala. Quien recibe usa solo el enlace, como en otro navegador.
 */
import { Asset, BASE_FEE, Horizon, Keypair, Networks, Operation, TransactionBuilder } from '@stellar/stellar-sdk'
import { STELLAR_HORIZON_URL, USDC_TESTNET_ISSUER, type HealthCheckResponse, type PaymentReceiptResponse } from '@paperpay/shared'
import { describe, expect, it, vi } from 'vitest'
import type { HorizonRecord } from '@/services/editorial'

const API = process.env.PAPERPAY_E2E_API_URL
const PAPER_ID = process.env.GIFT_PAPER_ID ?? 'stablecoins-emerging-markets'

const wallet = vi.hoisted(() => ({ keypair: null as Keypair | null }))
vi.mock('@/services/wallet', async () => {
  const sdk = await import('@stellar/stellar-sdk')
  return {
    isFreighter: () => true,
    signTxXdr: async (unsigned: string) => {
      const tx = sdk.TransactionBuilder.fromXDR(unsigned, sdk.Networks.TESTNET)
      tx.sign(wallet.keypair!)
      return tx.toXDR()
    },
  }
})

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

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

async function createPayer(): Promise<Keypair> {
  const payer = Keypair.random()
  const horizon = new Horizon.Server(STELLAR_HORIZON_URL)
  const usdc = new Asset('USDC', USDC_TESTNET_ISSUER)
  const friendbot = await fetch(`https://friendbot.stellar.org?addr=${encodeURIComponent(payer.publicKey())}`)
  if (!friendbot.ok) throw new Error(`Friendbot respondió ${friendbot.status}`)
  const account = await horizon.loadAccount(payer.publicKey())
  const tx = new TransactionBuilder(account, { fee: String(Number(BASE_FEE) * 10), networkPassphrase: Networks.TESTNET })
    .addOperation(Operation.changeTrust({ asset: usdc }))
    .addOperation(
      Operation.pathPaymentStrictReceive({ sendAsset: Asset.native(), sendMax: '50', destination: payer.publicKey(), destAsset: usdc, destAmount: '1', path: [] }),
    )
    .setTimeout(60)
    .build()
  tx.sign(payer)
  await horizon.submitTransaction(tx)
  return payer
}

describe.skipIf(!API)('e2e testnet: regalar un artículo por enlace', () => {
  it('quien regala paga gift → enlace → otro navegador lo abre sin wallet; quien paga no recibe el artículo', { timeout: 180_000 }, async () => {
    vi.stubEnv('NEXT_PUBLIC_API_URL', API!)
    const giver = { localStorage: memoryStorage(), sessionStorage: memoryStorage(), location: { search: '', origin: API! } }
    vi.stubGlobal('window', giver)
    const { canGift, sendGift, giftLink, parseGiftFragment, claimGift } = await import('@/services/gift')
    const { apiBackend } = await import('@/services/apiBackend')
    const { readsFromRecords } = await import('@/services/editorial')

    const health = (await (await fetch(`${API}/api/health`)).json()) as HealthCheckResponse
    expect(health.splitterContractId, 'la API no tiene SPLITTER_CONTRACT_ID').toMatch(/^C/)
    expect(await canGift(PAPER_ID), 'la API no anuncia giftUrl').toBe(true)

    wallet.keypair = await createPayer()
    const payer = wallet.keypair.publicKey()

    const started = Date.now()
    const gift = await sendGift(PAPER_ID, payer)
    const settleMs = Date.now() - started
    expect(gift.txHash).toMatch(/^[0-9a-f]{64}$/)
    // Quien regala no recibió acceso.
    expect((await apiBackend.getPaper(PAPER_ID)).status).toBe('locked')

    // Otro navegador: solo tiene el enlace.
    vi.stubGlobal('window', { localStorage: memoryStorage(), sessionStorage: memoryStorage(), location: { search: '', origin: API! } })
    const link = parseGiftFragment(new URL(giftLink(gift, API!)).hash)!
    const opened = await claimGift(link.txHash, link.secret)
    expect(opened.paperId).toBe(PAPER_ID)
    const reopened = await apiBackend.getPaper(PAPER_ID)
    expect(reopened.status).toBe('unlocked')

    const receipt = (await (await fetch(`${API}/api/receipts/${gift.txHash}`)).json()) as PaymentReceiptResponse
    expect(receipt).toMatchObject({ paperId: PAPER_ID, payer, reader: gift.reader, method: 'splitter', amountStroops: '5000000' })

    // Con otra llave (alguien que solo vio la transacción en la red) no se abre.
    const intruder = await claimGift(link.txHash, Keypair.random().secret()).then(() => 'abierto', (e: Error) => e.message)
    expect(intruder).not.toBe('abierto')

    await sleep(4_000) // Horizon ingiere el ledger un poco después que el RPC.
    const ops = (await (await fetch(`${STELLAR_HORIZON_URL}/transactions/${gift.txHash}/operations`)).json()) as { _embedded: { records: HorizonRecord[] } }
    const [entry] = readsFromRecords(ops._embedded.records, { treasury: health.treasuryPublicKey, splitterContractIds: health.splitterContractIds ?? [health.splitterContractId!] })
    expect(entry).toMatchObject({ kind: 'contract', reads: 1, publisherStroops: 4_900_000n, feeStroops: 100_000n, from: payer })

    process.stdout.write(`${JSON.stringify({ payer, reader: gift.reader, txHash: gift.txHash, settleMs, intruder }, null, 2)}\n`)
    // Para abrir el regalo a mano en un navegador (p. ej. un `next dev` local): imprime el enlace.
    const origin = process.env.PAPERPAY_E2E_GIFT_ORIGIN
    if (origin) process.stdout.write(`enlace: ${giftLink(gift, origin)}\n`)
  })
})
