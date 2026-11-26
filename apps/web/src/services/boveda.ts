import { STELLAR_HORIZON_URL, STELLAR_NETWORK_PASSPHRASE, STELLAR_RPC_URL } from '@paperpay/shared'
import { BOVEDA_CONTRACT_ID, contractFailure, normalizeCode, type BovedaFailure } from '@/lib/boveda'
import { signTxXdr } from '@/services/wallet'

/*
 * La pantalla /boveda habla directo con el RPC de Soroban (sin la API de PaperPay):
 *   1. saldo: simula balance() de la bóveda;
 *   2. reclamar: arma claim(claimer, code), lo simula (así un código mal falla antes de firmar),
 *      Freighter firma, se envía y se espera la confirmación.
 * Quien reclama es la cuenta fuente de la tx: paga el fee y su firma cubre el require_auth.
 */

export class BovedaError extends Error {
  constructor(
    public readonly failure: BovedaFailure | 'NO_ACCOUNT' | 'REJECTED' | 'TIMEOUT',
    detail?: string,
  ) {
    super(detail ?? failure)
  }
}

export interface ClaimResult {
  hash: string
  /** Stroops que llegaron a la cuenta. */
  amount: bigint
}

async function sdk() {
  return import('@stellar/stellar-sdk')
}

async function server() {
  const { rpc } = await sdk()
  return new rpc.Server(STELLAR_RPC_URL)
}

/** Saldo de la bóveda en stroops. */
export async function readVaultBalance(contractId = BOVEDA_CONTRACT_ID): Promise<bigint> {
  const { Account, BASE_FEE, Keypair, Operation, TransactionBuilder, rpc, scValToNative } = await sdk()
  // La fuente solo sirve para armar la simulación: el RPC no la carga ni la cobra.
  const tx = new TransactionBuilder(new Account(Keypair.random().publicKey(), '0'), {
    fee: BASE_FEE,
    networkPassphrase: STELLAR_NETWORK_PASSPHRASE,
  })
    .addOperation(Operation.invokeContractFunction({ contract: contractId, function: 'balance', args: [] }))
    .setTimeout(30)
    .build()
  const sim = await (await server()).simulateTransaction(tx)
  if (rpc.Api.isSimulationError(sim) || !sim.result) throw new Error('No se pudo leer el saldo')
  return BigInt(scValToNative(sim.result.retval) as bigint | number)
}

/** La cuenta existe en testnet (si no, no puede pagar el fee ni recibir XLM). */
export async function accountExists(address: string): Promise<boolean> {
  const res = await fetch(`${STELLAR_HORIZON_URL}/accounts/${encodeURIComponent(address)}`, {
    signal: AbortSignal.timeout(10_000),
  })
  if (res.status === 404) return false
  if (!res.ok) throw new Error(`Horizon respondió ${res.status}`)
  return true
}

/** Crea y fondea la cuenta en testnet con Friendbot (10 000 XLM de prueba). */
export async function activateWithFriendbot(address: string): Promise<void> {
  const res = await fetch(`https://friendbot.stellar.org/?addr=${encodeURIComponent(address)}`, {
    signal: AbortSignal.timeout(30_000),
  })
  // 400 = ya existía: también sirve.
  if (!res.ok && !(await accountExists(address))) throw new Error('Friendbot no pudo activar la cuenta')
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

/** Abre la bóveda: claim(claimer, code) firmado por `claimer`. */
export async function claimVault(
  claimer: string,
  input: string,
  contractId = BOVEDA_CONTRACT_ID,
): Promise<ClaimResult> {
  const { Address, BASE_FEE, Operation, TransactionBuilder, nativeToScVal, rpc, scValToNative } = await sdk()
  const rpcServer = await server()

  let account
  try {
    account = await rpcServer.getAccount(claimer)
  } catch {
    throw new BovedaError('NO_ACCOUNT')
  }
  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: STELLAR_NETWORK_PASSPHRASE })
    .addOperation(
      Operation.invokeContractFunction({
        contract: contractId,
        function: 'claim',
        args: [new Address(claimer).toScVal(), nativeToScVal(normalizeCode(input), { type: 'string' })],
      }),
    )
    .setTimeout(120)
    .build()

  const sim = await rpcServer.simulateTransaction(tx)
  if (rpc.Api.isSimulationError(sim)) throw new BovedaError(contractFailure(sim.error), sim.error)
  const prepared = rpc.assembleTransaction(tx, sim).build()

  let signedXdr: string
  try {
    signedXdr = await signTxXdr(prepared.toXDR(), claimer)
  } catch {
    throw new BovedaError('REJECTED')
  }
  const signed = TransactionBuilder.fromXDR(signedXdr, STELLAR_NETWORK_PASSPHRASE)
  // hash() da un Uint8Array y en el navegador no hay Buffer.
  const hash = Array.from(signed.hash(), (b) => b.toString(16).padStart(2, '0')).join('')

  let sent = await rpcServer.sendTransaction(signed)
  if (sent.status === 'ERROR') throw new BovedaError('OTHER', `La red rechazó la transacción (${sent.status})`)

  // ~5 s por ledger: hasta 60 s.
  for (let i = 0; i < 30; i++) {
    await sleep(2_000)
    const res = await rpcServer.getTransaction(hash)
    // TRY_AGAIN_LATER: el RPC no la tomó (o ya estaba aplicada; por eso se consulta antes de reenviar).
    if (res.status === rpc.Api.GetTransactionStatus.NOT_FOUND && sent.status === 'TRY_AGAIN_LATER' && i % 3 === 2) {
      sent = await rpcServer.sendTransaction(signed)
      continue
    }
    if (res.status === rpc.Api.GetTransactionStatus.SUCCESS) {
      const amount = res.returnValue ? BigInt(scValToNative(res.returnValue) as bigint | number) : 0n
      return { hash, amount }
    }
    if (res.status === rpc.Api.GetTransactionStatus.FAILED) {
      throw new BovedaError('OTHER', `La transacción falló en la red (${hash})`)
    }
  }
  throw new BovedaError('TIMEOUT', hash)
}
