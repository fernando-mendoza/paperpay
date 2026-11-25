import { Keypair, StrKey } from '@stellar/stellar-sdk';
import { GIFT_CLAIM_WINDOW_SECONDS, giftClaimMessage, type GiftClaimRequest, type X402PaymentSignatureHeader } from '@paperpay/shared';
import { ReceiptStore, receiptStore, sameReceipt, type Receipt } from './receipt.store';
import { SplitterError, SplitterService, splitterService, type SettledPayment } from './splitter.service';

/*
 * Regalo por enlace. Quien regala firma `gift(payer, reader, …)` con `reader` = una llave nueva que
 * su navegador crea y mete en el enlace (en el fragmento, que no llega al servidor). Quien abre el
 * enlace firma `giftClaimMessage` con esa llave y recibe el acceso. El hash y `reader` son públicos
 * en la red: sin la firma, cualquiera que mire la red podría reclamar el regalo.
 */

export class GiftError extends Error {
  public constructor(
    public readonly status: 400 | 403 | 404,
    public readonly code: 'INVALID_GIFT_CLAIM' | 'GIFT_CLAIM_EXPIRED' | 'GIFT_CLAIM_FORBIDDEN' | 'GIFT_NOT_FOUND',
    message: string,
  ) {
    super(message);
    this.name = 'GiftError';
  }
}

export class GiftService {
  public constructor(
    private readonly splitter: SplitterService = splitterService,
    private readonly store: ReceiptStore = receiptStore,
    private readonly now: () => number = Date.now,
  ) {}

  /** Envía el regalo firmado y guarda su recibo a nombre de `reader`. Quien paga no recibe el artículo. */
  public async send(payload: X402PaymentSignatureHeader, paperId: string): Promise<SettledPayment & { reader: string }> {
    if (!payload.signature || !payload.signerPublicKey || !this.splitter.isContractPayment(payload.signature)) {
      throw new SplitterError(400, 'El regalo debe ser una llamada gift del contrato de reparto, firmada por quien regala.');
    }
    const payer = payload.signerPublicKey;
    const { tx, reader } = this.splitter.validateSignedGift(payload.signature, payer, paperId);
    const settled = await this.splitter.settleGift(tx, payer, reader, paperId);
    this.record(settled, payer, reader);
    return { ...settled, reader };
  }

  /** Comprueba la firma de la llave del enlace y devuelve el recibo del regalo. */
  public async claim(request: Partial<GiftClaimRequest>): Promise<Receipt> {
    const { txHash, reader, signedAt, signature } = request;
    if (
      typeof txHash !== 'string' || !/^[0-9a-f]{64}$/i.test(txHash) ||
      typeof reader !== 'string' || !StrKey.isValidEd25519PublicKey(reader) ||
      typeof signedAt !== 'number' || !Number.isInteger(signedAt) ||
      typeof signature !== 'string'
    ) {
      throw new GiftError(400, 'INVALID_GIFT_CLAIM', 'El reclamo debe incluir txHash, reader, signedAt (segundos) y signature.');
    }
    if (Math.abs(this.now() / 1000 - signedAt) > GIFT_CLAIM_WINDOW_SECONDS) {
      throw new GiftError(400, 'GIFT_CLAIM_EXPIRED', 'La firma del enlace venció: vuelve a abrir el enlace de regalo.');
    }
    const sig = Buffer.from(signature, 'base64');
    if (sig.length !== 64 || !Keypair.fromPublicKey(reader).verify(Buffer.from(giftClaimMessage(txHash, signedAt)), sig)) {
      throw new GiftError(403, 'GIFT_CLAIM_FORBIDDEN', 'La firma no corresponde a la llave del enlace.');
    }

    const receipt = this.store.get(txHash) ?? (await this.fromNetwork(txHash));
    if (!receipt || receipt.reader === receipt.payer) {
      throw new GiftError(404, 'GIFT_NOT_FOUND', 'No hay un regalo de PaperPay con esa transacción.');
    }
    if (receipt.reader !== reader) {
      throw new GiftError(403, 'GIFT_CLAIM_FORBIDDEN', 'Ese enlace no es el de este regalo.');
    }
    return receipt;
  }

  /** Sin recibo en SQLite (escritura fallida o disco perdido): se busca el regalo en la red y se guarda. */
  private async fromNetwork(txHash: string): Promise<Receipt | undefined> {
    let found: SettledPayment | null;
    try {
      found = await this.splitter.findGift(txHash);
    } catch (err: unknown) {
      console.warn(`[giftService] No se pudo consultar ${txHash} en el RPC:`, err instanceof Error ? err.message : err);
      return undefined;
    }
    if (!found) return undefined;
    return this.record(found, found.receipt.payer, found.receipt.reader);
  }

  private record({ txHash, receipt, settledAt }: SettledPayment, payer: string, reader: string): Receipt | undefined {
    const wanted = { txHash, paperId: receipt.paperId, payer, reader, method: 'splitter' as const, amountStroops: receipt.amount.toString(), settledAt };
    try {
      const saved = this.store.record(wanted);
      if (!sameReceipt(saved, receipt.paperId, payer, reader)) {
        console.error(`[giftService] El hash ${txHash} ya tenía recibo de otro artículo o cuenta.`);
      }
      return saved;
    } catch (err: unknown) {
      // El cobro ya ocurrió en la red: el regalo se puede reclamar después desde el RPC.
      console.error(`[giftService] No se pudo guardar el recibo ${txHash}:`, err);
      return { ...wanted };
    }
  }
}

export const giftService = new GiftService();
