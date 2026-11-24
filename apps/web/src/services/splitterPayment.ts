import {
  STELLAR_HORIZON_URL,
  STELLAR_NETWORK_PASSPHRASE,
  USDC_TESTNET_ISSUER,
  type ApiErrorResponse,
  type SplitterPrepareResponse,
  type X402SplitterTerms,
} from '@paperpay/shared'
import { toStroops } from '@/lib/amounts'
import { API_URL } from '@/lib/api'
import { PaywallError } from '@/lib/errors'

/*
 * Pago por el contrato de reparto 98/2 (el 402 de la API trae extra.splitter):
 *   1. Horizon: la wallet existe y tiene el USDC del precio (así el error dice qué falta).
 *   2. La API arma y simula la invocación pay(payer, paperId, publisher, amount) (POST prepareUrl).
 *   3. Antes de abrir Freighter, se comprueba que el sobre es exactamente el pago que anunció el 402.
 * Freighter firma y el sobre firmado viaja en PAYMENT-SIGNATURE, igual que el pago clásico.
 */

interface HorizonBalance {
  asset_code?: string
  asset_issuer?: string
  balance: string
}

export interface ExpectedPayment {
  contractId: string
  publisher: string
  payer: string
  paperId: string
  amountStroops: string
}

export async function checkReaderFunds(address: string, amountStroops: string): Promise<void> {
  let res: Response
  try {
    res = await fetch(`${STELLAR_HORIZON_URL}/accounts/${encodeURIComponent(address)}`, {
      signal: AbortSignal.timeout(10_000),
    })
  } catch {
    return // Horizon no responde: la simulación de la API dirá si alcanza.
  }
  if (res.status === 404) {
    throw new PaywallError('INSUFFICIENT_FUNDS', 'La wallet todavía no existe en testnet: fondéala con XLM.')
  }
  if (!res.ok) return
  const { balances = [] } = (await res.json()) as { balances?: HorizonBalance[] }
  const usdc = balances.find((b) => b.asset_code === 'USDC' && b.asset_issuer === USDC_TESTNET_ISSUER)
  if (!usdc) throw new PaywallError('NO_TRUSTLINE')
  if (toStroops(usdc.balance) < BigInt(amountStroops)) throw new PaywallError('INSUFFICIENT_FUNDS')
}

export async function prepareSplitterPayment(splitter: X402SplitterTerms, payer: string): Promise<SplitterPrepareResponse> {
  // Solo rutas de la misma API: el 402 no puede mandar a la wallet a firmar algo de otro origen.
  if (!splitter.prepareUrl.startsWith('/api/')) {
    throw new PaywallError('PAYMENT_FAILED', 'Ruta de preparación inválida')
  }
  let res: Response
  try {
    res = await fetch(`${API_URL}${splitter.prepareUrl}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ payer }),
      signal: AbortSignal.timeout(20_000),
    })
  } catch {
    throw new PaywallError('PAYMENT_FAILED', 'API no disponible')
  }
  const body = (await res.json().catch(() => ({}))) as Partial<SplitterPrepareResponse & ApiErrorResponse>
  if (!res.ok || !body.transactionXdr || !body.networkPassphrase) {
    throw new PaywallError('PAYMENT_FAILED', body.message ?? `No se pudo preparar el pago (${res.status})`)
  }
  return body as SplitterPrepareResponse
}

/** Rechaza el sobre si no es una sola llamada `pay` del lector por este artículo, monto y editorial. */
export async function assertPreparedPayment(prepared: SplitterPrepareResponse, expected: ExpectedPayment): Promise<void> {
  const { Address, Transaction, TransactionBuilder, scValToNative, xdr } = await import('@stellar/stellar-sdk')
  const mismatch = (detail: string) =>
    new PaywallError('PAYMENT_FAILED', `El pago preparado no coincide con el anunciado (${detail})`)

  if (prepared.networkPassphrase !== STELLAR_NETWORK_PASSPHRASE) throw mismatch('red')
  let tx
  try {
    tx = TransactionBuilder.fromXDR(prepared.transactionXdr, STELLAR_NETWORK_PASSPHRASE)
  } catch {
    throw mismatch('XDR')
  }
  if (!(tx instanceof Transaction) || tx.source !== expected.payer || tx.operations.length !== 1) throw mismatch('sobre')
  const op = tx.operations[0]
  if (op.type !== 'invokeHostFunction' || (op.source && op.source !== expected.payer)) throw mismatch('operación')
  if (op.func.type !== 'hostFunctionTypeInvokeContract') throw mismatch('función')

  const isExpectedCall = (call: InstanceType<typeof xdr.InvokeContractArgs>): boolean => {
    const [payer, paperId, publisher, amount, ...rest] = call.args.map((arg) => scValToNative(arg) as unknown)
    return (
      Address.fromScAddress(call.contractAddress).toString() === expected.contractId &&
      call.functionName.toString() === 'pay' &&
      rest.length === 0 &&
      payer === expected.payer &&
      paperId === expected.paperId &&
      publisher === expected.publisher &&
      amount === BigInt(expected.amountStroops)
    )
  }
  if (!isExpectedCall(op.func.invokeContract)) throw mismatch('llamada')
  for (const entry of op.auth ?? []) {
    const root = entry.rootInvocation.function
    if (
      entry.credentials.type !== 'sorobanCredentialsSourceAccount' ||
      root.type !== 'sorobanAuthorizedFunctionTypeContractFn' ||
      !isExpectedCall(root.contractFn)
    ) {
      throw mismatch('autorización')
    }
  }
}
