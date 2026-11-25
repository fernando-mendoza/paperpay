/**
 * E2E en testnet del pago con memo (`?memo=` en la URL del artículo). SÍ mueve fondos de prueba: un
 * lector desechable paga 0.50 USDC por la transferencia clásica (Soroban no admite memo).
 *   PAPERPAY_E2E_API_URL=http://127.0.0.1:4011 pnpm --filter @paperpay/web test:e2e
 * Necesita una API con SELF_SETTLE=true (la de la VPS lo tiene). El memo de la prueba es
 * PAPERPAY_E2E_MEMO o "PAPERPAY-PRUEBA".
 */
import { Asset, BASE_FEE, Horizon, Keypair, Networks, Operation, TransactionBuilder } from '@stellar/stellar-sdk'
import { STELLAR_HORIZON_URL, USDC_TESTNET_ISSUER, type PaymentReceiptResponse } from '@paperpay/shared'
import { describe, expect, it, vi } from 'vitest'

const API = process.env.PAPERPAY_E2E_API_URL
const PAPER_ID = process.env.MEMO_PAPER_ID ?? 'autonomous-ai-micropayments'
const MEMO = process.env.PAPERPAY_E2E_MEMO ?? 'PAPERPAY-PRUEBA'

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

describe.skipIf(!API)('e2e testnet: pago con memo', () => {
  it('?memo= → transferencia clásica con ese memo en la red → artículo', { timeout: 180_000 }, async () => {
    vi.stubEnv('NEXT_PUBLIC_API_URL', API!)
    vi.stubGlobal('window', { localStorage: memoryStorage(), sessionStorage: memoryStorage(), location: { search: `?memo=${encodeURIComponent(MEMO)}`, origin: API! } })
    const { apiBackend } = await import('@/services/apiBackend')

    wallet.keypair = await createPayer()
    const payer = wallet.keypair.publicKey()
    const started = Date.now()
    const paid = await apiBackend.payForPaper(PAPER_ID, payer, () => {})
    const settleMs = Date.now() - started
    const { txHash } = paid.receipt
    expect(paid.paper.fullContentMarkdown.length).toBeGreaterThan(100)

    await sleep(3_000)
    const tx = (await (await fetch(`${STELLAR_HORIZON_URL}/transactions/${txHash}`)).json()) as { memo_type: string; memo: string; source_account: string }
    expect(tx).toMatchObject({ memo_type: 'text', memo: MEMO, source_account: payer })
    const receipt = (await (await fetch(`${API}/api/receipts/${txHash}`)).json()) as PaymentReceiptResponse
    expect(receipt).toMatchObject({ paperId: PAPER_ID, payer, method: 'classic' })

    process.stdout.write(`${JSON.stringify({ payer, txHash, memo: tx.memo, settleMs }, null, 2)}\n`)
  })
})
