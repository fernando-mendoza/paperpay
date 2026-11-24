import {
  Address,
  BASE_FEE,
  Contract,
  Keypair,
  StrKey,
  Transaction,
  TransactionBuilder,
  nativeToScVal,
  rpc,
  scValToNative,
  xdr,
} from '@stellar/stellar-sdk';
import { DEFAULT_PAPER_PRICE_STROOPS, STELLAR_NETWORK_PASSPHRASE } from '@paperpay/shared';
import { config } from '../config';

/*
 * Pago a través del contrato de reparto (contracts/splitter): la editorial recibe el 98% y la
 * tesorería el 2% en la misma transacción, y el contrato emite el evento `paid` con el artículo.
 *
 * Flujo: la API arma y simula la invocación de `pay` (preparePayment) → la wallet del lector la
 * firma → la API revisa lo firmado (validateSignedPayment), lo envía por RPC y solo da acceso si
 * la transacción se confirma con el comprobante `paid` esperado (settle).
 */

export const SPLITTER_PAY_FUNCTION = 'pay';

export class SplitterError extends Error {
  public constructor(
    public readonly status: 400 | 402 | 409 | 502 | 503 | 504,
    message: string,
    public readonly txHash?: string,
  ) {
    super(message);
    this.name = 'SplitterError';
  }
}

export interface SplitterTerms {
  contractId: string;
  publisher: string;
  amount: string;
}

/** Comprobante `paid` decodificado de un evento del contrato. */
export interface PaidReceipt {
  contractId: string;
  publisher: string;
  reader: string;
  paperId: string;
  payer: string;
  amount: bigint;
  fee: bigint;
}

/** Lo que el servicio usa del RPC de Soroban. `rpc.Server` lo cumple; las pruebas usan un doble. */
export type SplitterRpc = Pick<
  rpc.Server,
  'getAccount' | 'prepareTransaction' | 'sendTransaction' | 'getTransaction' | 'pollTransaction'
>;

export interface SplitterOptions {
  /** Vacío = pagos por contrato deshabilitados. */
  contractId: string;
  /** Cuenta de la editorial que recibe el 98% (hoy, una editorial demo para todo el catálogo). */
  publisher: string;
  rpcUrl: string;
  networkPassphrase: string;
  amount?: string;
  pollAttempts?: number;
  /** Un pago ya confirmado solo se acepta otra vez (respuesta perdida) si tiene menos de esto. */
  recoveryMaxAgeSeconds?: number;
  rpc?: SplitterRpc;
  now?: () => number;
}

export class SplitterService {
  private client: SplitterRpc | null;
  private readonly amount: string;
  private readonly pollAttempts: number;
  private readonly recoveryMaxAgeSeconds: number;
  private readonly now: () => number;

  public constructor(private readonly opts: SplitterOptions) {
    this.client = opts.rpc ?? null;
    this.amount = opts.amount ?? DEFAULT_PAPER_PRICE_STROOPS;
    this.pollAttempts = opts.pollAttempts ?? 30;
    this.recoveryMaxAgeSeconds = opts.recoveryMaxAgeSeconds ?? 300;
    this.now = opts.now ?? Date.now;
  }

  public get enabled(): boolean {
    return this.opts.contractId !== '';
  }

  public terms(): SplitterTerms {
    return { contractId: this.opts.contractId, publisher: this.opts.publisher, amount: this.amount };
  }

  /** Arma y simula `pay(payer, paperId, publisher, amount)`; devuelve el XDR listo para firmar. */
  public async preparePayment(paperId: string, payer: string): Promise<string> {
    this.requireEnabled();
    if (!StrKey.isValidEd25519PublicKey(payer)) {
      throw new SplitterError(400, 'La wallet del lector no es una dirección Stellar válida (G...).');
    }

    const server = this.rpc();
    let account;
    try {
      account = await server.getAccount(payer);
    } catch {
      throw new SplitterError(402, 'Esa wallet todavía no existe en Stellar testnet: fondéala con XLM antes de pagar.');
    }

    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.opts.networkPassphrase,
    })
      .addOperation(new Contract(this.opts.contractId).call(SPLITTER_PAY_FUNCTION, ...this.payArgs(payer, paperId)))
      .setTimeout(300)
      .build();

    try {
      const prepared = await server.prepareTransaction(tx);
      return prepared.toXDR();
    } catch (err: unknown) {
      console.warn('[splitter] La simulación de pay falló:', err instanceof Error ? err.message : err);
      throw new SplitterError(
        402,
        'No se pudo preparar el pago: la wallet necesita 0.50 USDC (con trustline) y un poco de XLM para la comisión de red.',
      );
    }
  }

  /** ¿Es un sobre que invoca un contrato? Sirve para enrutar entre el pago clásico y este. */
  public isContractPayment(transactionXdr: string): boolean {
    try {
      const tx = TransactionBuilder.fromXDR(transactionXdr, this.opts.networkPassphrase);
      return tx instanceof Transaction && tx.operations.length > 0 && tx.operations[0].type === 'invokeHostFunction';
    } catch {
      return false;
    }
  }

  /**
   * Revisa, sin tocar la red, que el sobre firmado sea exactamente `pay` de nuestro contrato
   * para este artículo, con la editorial y el monto que fija la API, y firmado por el lector.
   */
  public validateSignedPayment(transactionXdr: string, signer: string, paperId: string): Transaction {
    this.requireEnabled();
    let parsed;
    try {
      parsed = TransactionBuilder.fromXDR(transactionXdr, this.opts.networkPassphrase);
    } catch {
      throw new SplitterError(400, 'El campo signature no es un XDR de transacción válido.');
    }
    if (!(parsed instanceof Transaction)) {
      throw new SplitterError(400, 'No se aceptan transacciones fee-bump.');
    }
    const tx = parsed;

    if (!StrKey.isValidEd25519PublicKey(signer) || tx.source !== signer) {
      throw new SplitterError(400, 'La wallet firmante no coincide con el origen de la transacción.');
    }
    if (tx.operations.length !== 1) {
      throw new SplitterError(400, 'La transacción debe tener una sola operación.');
    }
    const op = tx.operations[0];
    if (op.type !== 'invokeHostFunction' || (op.source && op.source !== signer)) {
      throw new SplitterError(400, 'La operación no es una invocación de contrato del lector.');
    }
    const func = op.func;
    if (func.type !== 'hostFunctionTypeInvokeContract') {
      throw new SplitterError(400, 'La operación no invoca una función de contrato.');
    }

    this.requirePayCall(func.invokeContract, signer, paperId);
    for (const entry of op.auth ?? []) {
      if (entry.credentials.type !== 'sorobanCredentialsSourceAccount') {
        throw new SplitterError(400, 'La autorización debe ser la de la cuenta que firma la transacción.');
      }
      const root = entry.rootInvocation.function;
      if (root.type !== 'sorobanAuthorizedFunctionTypeContractFn') {
        throw new SplitterError(400, 'La autorización no corresponde al pago.');
      }
      this.requirePayCall(root.contractFn, signer, paperId);
    }

    const keypair = Keypair.fromPublicKey(signer);
    const hash = tx.hash();
    const signed = tx.signatures.some(
      (sig) =>
        Buffer.from(sig.hint.toBytes()).equals(keypair.signatureHint()) &&
        keypair.verify(hash, Buffer.from(sig.signature.toBytes())),
    );
    if (!signed) {
      throw new SplitterError(400, 'La transacción no está firmada por la wallet del lector.');
    }
    return tx;
  }

  /**
   * Envía el pago y espera la confirmación con su comprobante `paid`.
   * Si ese mismo sobre ya está confirmado (el lector reintenta porque perdió la respuesta), no se
   * reenvía: se da el acceso solo si el pago es reciente (misma ventana que la verificación por hash
   * de Pollar); el sobre de un pago viejo es público en la red y no debe abrir el artículo a otros.
   */
  public async settle(tx: Transaction, signer: string, paperId: string): Promise<{ txHash: string; receipt: PaidReceipt }> {
    this.requireEnabled();
    const server = this.rpc();

    const existing = await server.getTransaction(Buffer.from(tx.hash()).toString('hex'));
    if (existing.status === rpc.Api.GetTransactionStatus.SUCCESS) {
      const ageSeconds = this.now() / 1000 - existing.createdAt;
      if (ageSeconds > this.recoveryMaxAgeSeconds) {
        throw new SplitterError(409, 'Ese pago ya se usó. Para leer el artículo otra vez, usa el acceso que se guardó al pagar.', existing.txHash);
      }
      return { txHash: existing.txHash, receipt: this.requireReceipt(existing, signer, paperId) };
    }
    if (existing.status === rpc.Api.GetTransactionStatus.FAILED) {
      throw new SplitterError(402, 'Ese pago ya falló en la red: revisa tu saldo de USDC y vuelve a pagar.', existing.txHash);
    }

    const sent = await server.sendTransaction(tx);
    const txHash = sent.hash;

    if (sent.status === 'ERROR') {
      const code = sent.errorResult?.result.type ?? 'desconocido';
      console.warn(`[splitter] sendTransaction rechazó ${txHash}: ${code}`);
      throw new SplitterError(402, `La red rechazó el pago antes de procesarlo (${code}).`, txHash);
    }
    if (sent.status === 'TRY_AGAIN_LATER') {
      throw new SplitterError(503, 'La red está saturada: intenta pagar de nuevo en unos segundos.', txHash);
    }

    const result = await server.pollTransaction(txHash, { attempts: this.pollAttempts });
    if (result.status === rpc.Api.GetTransactionStatus.NOT_FOUND) {
      throw new SplitterError(504, 'El pago sigue pendiente de confirmación. Revisa tu wallet antes de reintentar.', txHash);
    }
    if (result.status === rpc.Api.GetTransactionStatus.FAILED) {
      throw new SplitterError(402, 'La red rechazó el pago: revisa tu saldo de USDC.', txHash);
    }

    return { txHash, receipt: this.requireReceipt(result, signer, paperId) };
  }

  private requireReceipt(result: rpc.Api.GetSuccessfulTransactionResponse, signer: string, paperId: string): PaidReceipt {
    const events = result.events?.contractEventsXdr.flat() ?? [];
    const receipt = parsePaidReceipts(events).find(
      (r) =>
        r.contractId === this.opts.contractId &&
        r.paperId === paperId &&
        r.payer === signer &&
        r.reader === signer &&
        r.publisher === this.opts.publisher &&
        r.amount === BigInt(this.amount),
    );
    if (!receipt) {
      throw new SplitterError(502, 'El pago se confirmó, pero sin el comprobante esperado del artículo.', result.txHash);
    }
    return receipt;
  }

  private payArgs(payer: string, paperId: string): xdr.ScVal[] {
    return [
      nativeToScVal(payer, { type: 'address' }),
      nativeToScVal(paperId, { type: 'string' }),
      nativeToScVal(this.opts.publisher, { type: 'address' }),
      nativeToScVal(BigInt(this.amount), { type: 'i128' }),
    ];
  }

  private requirePayCall(call: xdr.InvokeContractArgs, signer: string, paperId: string): void {
    const contractId = Address.fromScAddress(call.contractAddress).toString();
    if (contractId !== this.opts.contractId) {
      throw new SplitterError(400, 'La transacción no invoca el contrato de reparto de PaperPay.');
    }
    if (call.functionName.toString() !== SPLITTER_PAY_FUNCTION) {
      throw new SplitterError(400, `Solo se acepta la función ${SPLITTER_PAY_FUNCTION} del contrato.`);
    }
    const args = call.args.map((arg) => scValToNative(arg) as unknown);
    const [payer, paper, publisher, amount] = args;
    if (args.length !== 4 || payer !== signer) {
      throw new SplitterError(400, 'El pago debe salir de la wallet que firma.');
    }
    if (paper !== paperId) {
      throw new SplitterError(400, 'El pago es por otro artículo.');
    }
    if (publisher !== this.opts.publisher) {
      throw new SplitterError(400, 'El pago no va a la editorial de este artículo.');
    }
    if (amount !== BigInt(this.amount)) {
      throw new SplitterError(400, 'El monto del pago no es el precio del artículo.');
    }
  }

  private requireEnabled(): void {
    if (!this.enabled) throw new SplitterError(400, 'Los pagos por contrato no están habilitados en esta API.');
  }

  private rpc(): SplitterRpc {
    if (!this.client) {
      this.client = new rpc.Server(this.opts.rpcUrl, { allowHttp: this.opts.rpcUrl.startsWith('http://') });
    }
    return this.client;
  }
}

/** Decodifica los eventos `paid` (topics `["paid", publisher, reader]`); ignora cualquier otro. */
export function parsePaidReceipts(events: xdr.ContractEvent[]): PaidReceipt[] {
  const receipts: PaidReceipt[] = [];
  for (const event of events) {
    try {
      const contract = event.contractId;
      if (!contract || event.body.type !== 'v0') continue;
      const body = event.body.v0;
      const topics = body.topics.map((t) => scValToNative(t) as unknown);
      if (topics.length !== 3 || topics[0] !== 'paid') continue;
      const data = scValToNative(body.data) as Record<string, unknown>;
      receipts.push({
        contractId: StrKey.encodeContract(Buffer.from(contract.toBytes())),
        publisher: String(topics[1]),
        reader: String(topics[2]),
        paperId: String(data.paper_id),
        payer: String(data.payer),
        amount: BigInt(data.amount as bigint),
        fee: BigInt(data.fee as bigint),
      });
    } catch {
      // Un evento con otra forma no es un comprobante: se ignora.
    }
  }
  return receipts;
}

export const splitterService = new SplitterService({
  contractId: config.splitterContractId,
  publisher: config.splitterPublisher,
  rpcUrl: config.stellarRpcUrl,
  networkPassphrase: STELLAR_NETWORK_PASSPHRASE,
});
