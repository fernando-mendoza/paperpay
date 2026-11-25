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
import { createHmac, randomInt, timingSafeEqual } from 'node:crypto';
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
/** Regalo: cobra a `payer` y deja el acceso a nombre de `reader` (solo firma `payer`). */
export const SPLITTER_GIFT_FUNCTION = 'gift';
type SplitterFunction = typeof SPLITTER_PAY_FUNCTION | typeof SPLITTER_GIFT_FUNCTION;

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
  /** Un pago ya confirmado se acepta otra vez sin comprobante (respuesta perdida) si tiene menos de esto. */
  recoveryMaxAgeSeconds?: number;
  /**
   * Llave del comprobante que `/prepare` entrega junto al sobre. Vacía = sin comprobantes: un pago
   * confirmado hace más de `recoveryMaxAgeSeconds` ya no da acceso.
   */
  claimSecret?: string;
  /** Con comprobante, un pago confirmado se acepta otra vez hasta esta edad (lo que dura el acceso). */
  claimMaxAgeSeconds?: number;
  rpc?: SplitterRpc;
  now?: () => number;
}

export interface SettledPayment {
  txHash: string;
  receipt: PaidReceipt;
  settledAt: string;
}

/** Lo que devuelve `/prepare`: el sobre para firmar y el comprobante que solo recibe quien lo pidió. */
export interface PreparedPayment {
  transactionXdr: string;
  claim?: string;
}

export class SplitterService {
  private client: SplitterRpc | null;
  private readonly amount: string;
  private readonly pollAttempts: number;
  private readonly recoveryMaxAgeSeconds: number;
  private readonly claimMaxAgeSeconds: number;
  private readonly now: () => number;

  public constructor(private readonly opts: SplitterOptions) {
    this.client = opts.rpc ?? null;
    this.amount = opts.amount ?? DEFAULT_PAPER_PRICE_STROOPS;
    this.pollAttempts = opts.pollAttempts ?? 30;
    this.recoveryMaxAgeSeconds = opts.recoveryMaxAgeSeconds ?? 300;
    this.claimMaxAgeSeconds = opts.claimMaxAgeSeconds ?? 24 * 60 * 60;
    this.now = opts.now ?? Date.now;
  }

  public get enabled(): boolean {
    return this.opts.contractId !== '';
  }

  public terms(): SplitterTerms {
    return { contractId: this.opts.contractId, publisher: this.opts.publisher, amount: this.amount };
  }

  /**
   * Arma y simula `pay(payer, paperId, publisher, amount)` o, con `reader`, el regalo
   * `gift(payer, reader, paperId, publisher, amount)`. Devuelve el XDR listo para firmar y su
   * comprobante (ver `settle`).
   */
  public async preparePayment(paperId: string, payer: string, reader?: string): Promise<PreparedPayment> {
    this.requireEnabled();
    if (!StrKey.isValidEd25519PublicKey(payer)) {
      throw new SplitterError(400, 'La wallet del lector no es una dirección Stellar válida (G...).');
    }
    if (reader !== undefined && (!StrKey.isValidEd25519PublicKey(reader) || reader === payer)) {
      throw new SplitterError(400, 'Quien recibe el regalo debe ser otra dirección Stellar válida (G...).');
    }
    const call =
      reader === undefined
        ? new Contract(this.opts.contractId).call(SPLITTER_PAY_FUNCTION, ...this.payArgs(payer, paperId))
        : new Contract(this.opts.contractId).call(SPLITTER_GIFT_FUNCTION, ...this.giftArgs(payer, reader, paperId));

    const server = this.rpc();
    let account;
    try {
      account = await server.getAccount(payer);
    } catch {
      throw new SplitterError(402, 'Esa wallet todavía no existe en Stellar testnet: fondéala con XLM antes de pagar.');
    }

    // La comisión de inclusión lleva hasta 999 stroops (0.0001 XLM) al azar: así dos pedidos de
    // /prepare con la misma wallet en el mismo segundo no producen el mismo sobre (ni el mismo
    // comprobante).
    const tx = new TransactionBuilder(account, {
      fee: String(Number(BASE_FEE) + randomInt(0, 1000)),
      networkPassphrase: this.opts.networkPassphrase,
    })
      .addOperation(call)
      .setTimeout(300)
      .build();

    try {
      const prepared = await server.prepareTransaction(tx);
      return { transactionXdr: prepared.toXDR(), claim: this.claimFor(prepared.hash()) };
    } catch (err: unknown) {
      console.warn(`[splitter] La simulación de ${reader === undefined ? 'pay' : 'gift'} falló:`, err instanceof Error ? err.message : err);
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
    return this.validateSigned(transactionXdr, signer, paperId, SPLITTER_PAY_FUNCTION).tx;
  }

  /** Igual que `validateSignedPayment`, pero para `gift`. Devuelve también quién recibe el acceso. */
  public validateSignedGift(transactionXdr: string, signer: string, paperId: string): { tx: Transaction; reader: string } {
    return this.validateSigned(transactionXdr, signer, paperId, SPLITTER_GIFT_FUNCTION);
  }

  private validateSigned(
    transactionXdr: string,
    signer: string,
    paperId: string,
    fn: SplitterFunction,
  ): { tx: Transaction; reader: string } {
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

    const reader = this.requireCall(func.invokeContract, signer, paperId, fn);
    for (const entry of op.auth ?? []) {
      if (entry.credentials.type !== 'sorobanCredentialsSourceAccount') {
        throw new SplitterError(400, 'La autorización debe ser la de la cuenta que firma la transacción.');
      }
      const root = entry.rootInvocation.function;
      if (root.type !== 'sorobanAuthorizedFunctionTypeContractFn') {
        throw new SplitterError(400, 'La autorización no corresponde al pago.');
      }
      this.requireCall(root.contractFn, signer, paperId, fn);
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
    return { tx, reader };
  }

  /**
   * Envía el pago y espera la confirmación con su comprobante `paid`.
   * Si ese mismo sobre ya está confirmado (el lector reintenta porque perdió la respuesta), no se
   * reenvía. El sobre confirmado es público en la red, así que no basta para abrir el artículo:
   * - sin comprobante, solo si el pago es reciente (misma ventana que la verificación por hash de Pollar);
   * - con el comprobante que `/prepare` le dio a quien pidió ese sobre, hasta lo que dura el acceso.
   */
  public async settle(
    tx: Transaction,
    signer: string,
    paperId: string,
    claim?: string,
  ): Promise<SettledPayment> {
    return this.confirm(tx, signer, signer, paperId, (existing) => {
      const ageSeconds = this.now() / 1000 - existing.createdAt;
      if (ageSeconds > this.recoveryMaxAgeSeconds) {
        if (!this.isValidClaim(tx.hash(), claim)) {
          throw new SplitterError(409, 'Ese pago ya se usó. Para leer el artículo otra vez, usa el acceso que se guardó al pagar.', existing.txHash);
        }
        if (ageSeconds > this.claimMaxAgeSeconds) {
          throw new SplitterError(409, 'Ese pago tiene más de 24 horas: el acceso que daba ya venció.', existing.txHash);
        }
      }
    });
  }

  /**
   * Envía el regalo y espera su comprobante `paid` a nombre de `reader`. Reenviar un regalo ya
   * confirmado solo devuelve su recibo: quien paga no recibe el artículo, y la llave del enlace
   * nunca pasa por la API.
   */
  public async settleGift(tx: Transaction, payer: string, reader: string, paperId: string): Promise<SettledPayment> {
    return this.confirm(tx, payer, reader, paperId, () => {});
  }

  /**
   * Busca en la red un regalo ya confirmado de nuestro contrato (para reclamarlo si su recibo no
   * quedó en SQLite). El RPC solo retiene unos días de transacciones.
   */
  public async findGift(txHash: string): Promise<SettledPayment | null> {
    if (!this.enabled) return null;
    const found = await this.rpc().getTransaction(txHash.toLowerCase());
    if (found.status !== rpc.Api.GetTransactionStatus.SUCCESS) return null;
    const receipt = this.paidReceipts(found).find((r) => r.reader !== r.payer);
    return receipt ? { txHash: found.txHash, receipt, settledAt: ledgerTime(found.createdAt) } : null;
  }

  private async confirm(
    tx: Transaction,
    payer: string,
    reader: string,
    paperId: string,
    checkAlreadyConfirmed: (existing: rpc.Api.GetSuccessfulTransactionResponse) => void,
  ): Promise<SettledPayment> {
    this.requireEnabled();
    const server = this.rpc();

    const existing = await server.getTransaction(Buffer.from(tx.hash()).toString('hex'));
    if (existing.status === rpc.Api.GetTransactionStatus.SUCCESS) {
      checkAlreadyConfirmed(existing);
      return { txHash: existing.txHash, receipt: this.requireReceipt(existing, payer, reader, paperId), settledAt: ledgerTime(existing.createdAt) };
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

    return { txHash, receipt: this.requireReceipt(result, payer, reader, paperId), settledAt: ledgerTime(result.createdAt) };
  }

  /** HMAC del hash del sobre: la red nunca lo ve, solo quien recibió la respuesta de `/prepare`. */
  private claimFor(txHash: Uint8Array): string | undefined {
    if (!this.opts.claimSecret) return undefined;
    return createHmac('sha256', this.opts.claimSecret)
      .update(`paperpay:splitter-claim:${this.opts.contractId}:`)
      .update(txHash)
      .digest('base64url');
  }

  private isValidClaim(txHash: Uint8Array, claim: string | undefined): boolean {
    const expected = this.claimFor(txHash);
    if (!expected || typeof claim !== 'string') return false;
    const given = Buffer.from(claim);
    const wanted = Buffer.from(expected);
    return given.length === wanted.length && timingSafeEqual(given, wanted);
  }

  /** Comprobantes `paid` de nuestro contrato, con nuestra editorial y nuestro precio. */
  private paidReceipts(result: rpc.Api.GetSuccessfulTransactionResponse): PaidReceipt[] {
    const events = result.events?.contractEventsXdr.flat() ?? [];
    return parsePaidReceipts(events).filter(
      (r) => r.contractId === this.opts.contractId && r.publisher === this.opts.publisher && r.amount === BigInt(this.amount),
    );
  }

  private requireReceipt(
    result: rpc.Api.GetSuccessfulTransactionResponse,
    payer: string,
    reader: string,
    paperId: string,
  ): PaidReceipt {
    const receipt = this.paidReceipts(result).find((r) => r.paperId === paperId && r.payer === payer && r.reader === reader);
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

  private giftArgs(payer: string, reader: string, paperId: string): xdr.ScVal[] {
    const [payerArg, ...rest] = this.payArgs(payer, paperId);
    return [payerArg, nativeToScVal(reader, { type: 'address' }), ...rest];
  }

  /** Revisa una llamada `pay` o `gift` y devuelve quién recibe el acceso. */
  private requireCall(call: xdr.InvokeContractArgs, signer: string, paperId: string, fn: SplitterFunction): string {
    const contractId = Address.fromScAddress(call.contractAddress).toString();
    if (contractId !== this.opts.contractId) {
      throw new SplitterError(400, 'La transacción no invoca el contrato de reparto de PaperPay.');
    }
    if (call.functionName.toString() !== fn) {
      throw new SplitterError(400, `Solo se acepta la función ${fn} del contrato.`);
    }
    // pay(payer, paper, publisher, amount) · gift(payer, reader, paper, publisher, amount)
    const args = call.args.map((arg) => scValToNative(arg) as unknown);
    const isGift = fn === SPLITTER_GIFT_FUNCTION;
    const [payer, ...rest] = args;
    const reader = isGift ? rest.shift() : signer;
    const [paper, publisher, amount] = rest;
    if (rest.length !== 3 || payer !== signer) {
      throw new SplitterError(400, 'El pago debe salir de la wallet que firma.');
    }
    if (typeof reader !== 'string' || !StrKey.isValidEd25519PublicKey(reader) || (isGift && reader === signer)) {
      throw new SplitterError(400, 'Quien recibe el regalo debe ser otra cuenta Stellar.');
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
    return reader;
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
  // Derivada del JWT_SECRET: rotarlo invalida los comprobantes, igual que los accesos.
  claimSecret: createHmac('sha256', config.jwtSecret).update('paperpay:splitter-claim-key').digest('hex'),
});

/** Hora del ledger (segundos Unix de RPC) en ISO 8601. */
function ledgerTime(createdAt: number): string {
  return new Date(createdAt * 1000).toISOString();
}
