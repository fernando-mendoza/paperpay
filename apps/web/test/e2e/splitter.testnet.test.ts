/**
 * E2E en testnet del sitio pagando por el contrato de reparto. SÍ mueve fondos de prueba: crea un
 * lector desechable (Friendbot + 1 USDC del DEX de testnet) y paga 0.50 USDC.
 *
 * Necesita una API local con el contrato activo (ver apps/api/README.md):
 *   PAPERPAY_E2E_API_URL=http://127.0.0.1:4011 pnpm --filter @paperpay/web test:e2e
 *
 * Corre el código real del sitio (apiBackend + panel editorial). Solo Freighter está simulado: la
 * firma la hace la llave del lector, igual que Freighter firmaría el sobre que le pasa el sitio.
 */
import { Asset, BASE_FEE, Horizon, Keypair, Networks, Operation, TransactionBuilder } from '@stellar/stellar-sdk'
import { STELLAR_HORIZON_URL, USDC_TESTNET_ISSUER, type HealthCheckResponse } from '@paperpay/shared'
import { describe, expect, it, vi } from 'vitest'
import type { HorizonRecord } from '@/services/editorial'

const API = process.env.PAPERPAY_E2E_API_URL
const PAPER_ID = process.env.PAPER_ID ?? 'crispr-nanomedicine-biotech'

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

async function createReader(): Promise<Keypair> {
  const reader = Keypair.random()
  const horizon = new Horizon.Server(STELLAR_HORIZON_URL)
  const usdc = new Asset('USDC', USDC_TESTNET_ISSUER)
  const friendbot = await fetch(`https://friendbot.stellar.org?addr=${encodeURIComponent(reader.publicKey())}`)
  if (!friendbot.ok) throw new Error(`Friendbot respondió ${friendbot.status}`)
  const account = await horizon.loadAccount(reader.publicKey())
  const tx = new TransactionBuilder(account, { fee: String(Number(BASE_FEE) * 10), networkPassphrase: Networks.TESTNET })
    .addOperation(Operation.changeTrust({ asset: usdc }))
    .addOperation(
      Operation.pathPaymentStrictReceive({
        sendAsset: Asset.native(),
        sendMax: '50',
        destination: reader.publicKey(),
        destAsset: usdc,
        destAmount: '1',
        path: [],
      }),
    )
    .setTimeout(60)
    .build()
  tx.sign(reader)
  await horizon.submitTransaction(tx)
  return reader
}

describe.skipIf(!API)('e2e testnet: el sitio paga por el contrato de reparto', () => {
  it('402 → /prepare → firma → artículo; el panel lo cuenta como lectura por contrato', { timeout: 180_000 }, async () => {
    vi.stubEnv('NEXT_PUBLIC_API_URL', API!)
    vi.stubGlobal('window', { localStorage: memoryStorage(), sessionStorage: memoryStorage(), location: { search: '' } })
    const { apiBackend } = await import('@/services/apiBackend')
    const { getEditorialStats, readsFromRecords } = await import('@/services/editorial')

    const health = (await (await fetch(`${API}/api/health`)).json()) as HealthCheckResponse
    expect(health.splitterContractId, 'la API local no tiene SPLITTER_CONTRACT_ID').toMatch(/^C/)

    wallet.keypair = await createReader()
    const reader = wallet.keypair.publicKey()

    expect((await apiBackend.getPaper(PAPER_ID)).status).toBe('locked')
    const started = Date.now()
    const paid = await apiBackend.payForPaper(PAPER_ID, reader, () => {})
    const settleMs = Date.now() - started
    const { txHash } = paid.receipt
    expect(txHash).toMatch(/^[0-9a-f]{64}$/)
    expect(paid.paper.fullContentMarkdown.length).toBeGreaterThan(100)

    // Recargar el artículo: entra con el JWT que guardó el pago, sin pagar otra vez.
    const reopened = await apiBackend.getPaper(PAPER_ID)
    expect(reopened.status).toBe('unlocked')

    await sleep(4_000) // Horizon ingiere el ledger un poco después que el RPC.
    const ops = (await (await fetch(`${STELLAR_HORIZON_URL}/transactions/${txHash}/operations`)).json()) as {
      _embedded: { records: HorizonRecord[] }
    }
    const [entry] = readsFromRecords(ops._embedded.records, {
      treasury: health.treasuryPublicKey,
      splitterContractId: health.splitterContractId,
    })
    expect(entry).toMatchObject({ kind: 'contract', reads: 1, amount: 0.5, publisherStroops: 4_900_000n, feeStroops: 100_000n, from: reader })

    const stats = await getEditorialStats()
    expect(stats.recent[0]).toMatchObject({ hash: txHash, kind: 'contract', reads: 1 })

    // stdout directo: el reporter de vitest oculta console.log de las pruebas que pasan.
    const panel = { reads: stats.reads, contractReads: stats.contractReads, totalUsdc: stats.totalUsdc, publisherUsdc: stats.publisherUsdc, feeUsdc: stats.feeUsdc }
    process.stdout.write(`${JSON.stringify({ reader, txHash, settleMs, panel }, null, 2)}\n`)
  })
})
