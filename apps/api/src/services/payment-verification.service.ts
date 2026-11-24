import {
  DEFAULT_PAPER_PRICE_STROOPS,
  STELLAR_NETWORK,
  USDC_TESTNET_ISSUER,
} from '@paperpay/shared';
import { config } from '../config';
import { ReceiptStore, receiptStore, sameReceipt, type Receipt } from './receipt.store';

interface HorizonTransaction {
  successful?: boolean;
  created_at?: string;
  source_account?: string;
}

interface HorizonPaymentOperation {
  type?: string;
  from?: string;
  to?: string;
  amount?: string;
  asset_type?: string;
  asset_code?: string;
  asset_issuer?: string;
}

interface HorizonCollection<T> {
  _embedded?: { records?: T[] };
}

export interface VerifiedPayment {
  txHash: string;
  paperId: string;
  signerPublicKey: string;
}

export class PaymentVerificationError extends Error {
  public constructor(
    public readonly status: 400 | 402 | 409 | 502,
    message: string,
  ) {
    super(message);
    this.name = 'PaymentVerificationError';
  }
}

/**
 * Verifica los pagos que envía Pollar. El recibo (hash → artículo y wallet) queda en SQLite
 * (`ReceiptStore`), así que un reinicio no permite usar el mismo hash para otro artículo.
 * Varias instancias necesitarían compartir el archivo o pasar a un almacén de red.
 */
export class PaymentVerificationService {
  private readonly pending = new Map<string, Promise<VerifiedPayment>>();

  public constructor(private readonly store: ReceiptStore = receiptStore) {}

  public async verifyByHash(input: VerifiedPayment): Promise<VerifiedPayment> {
    this.validateInput(input);

    const stored = this.store.get(input.txHash);
    if (stored) return this.reuseReceipt(stored, input);

    const inFlight = this.pending.get(input.txHash);
    if (inFlight) {
      return this.requireSameReceipt(await inFlight, input);
    }

    const verification = this.verifyOnHorizon(input)
      .then((settledAt) => {
        const saved = this.store.record({
          txHash: input.txHash,
          paperId: input.paperId,
          payer: input.signerPublicKey,
          method: 'pollar',
          amountStroops: String(DEFAULT_PAPER_PRICE_STROOPS),
          settledAt,
        });
        if (!sameReceipt(saved, input.paperId, input.signerPublicKey)) throw this.alreadyUsed();
        return input;
      })
      .finally(() => this.pending.delete(input.txHash));

    this.pending.set(input.txHash, verification);
    return verification;
  }

  public clearReceiptsForTests(): void {
    this.store.clearForTests();
    this.pending.clear();
  }

  /**
   * Reintento con un hash ya verificado: solo para el mismo artículo y wallet, y solo dentro de
   * la misma ventana que la primera verificación. El hash y la wallet son públicos en la red:
   * sin este límite, cualquiera que los copie tendría acceso para siempre.
   */
  private reuseReceipt(stored: Receipt, input: VerifiedPayment): VerifiedPayment {
    if (stored.method !== 'pollar' || !sameReceipt(stored, input.paperId, input.signerPublicKey)) {
      throw this.alreadyUsed();
    }
    const age = Date.now() - Date.parse(stored.settledAt);
    if (age > config.paymentVerificationMaxAgeSeconds * 1000) {
      throw new PaymentVerificationError(409, 'Este pago ya se verificó y su ventana de verificación terminó. Si perdiste el acceso, revisa tu wallet antes de pagar de nuevo.');
    }
    return input;
  }

  private alreadyUsed(): PaymentVerificationError {
    return new PaymentVerificationError(409, 'Esta transacción ya fue usada para desbloquear otro artículo o firmante.');
  }

  private validateInput(input: VerifiedPayment): void {
    if (!/^[0-9a-f]{64}$/i.test(input.txHash)) {
      throw new PaymentVerificationError(400, 'txHash debe ser un hash Stellar hexadecimal de 64 caracteres.');
    }
    if (!input.paperId || !input.signerPublicKey) {
      throw new PaymentVerificationError(400, 'Se requieren el artículo y la clave pública del firmante.');
    }
  }

  private requireSameReceipt(receipt: VerifiedPayment, input: VerifiedPayment): VerifiedPayment {
    if (receipt.paperId === input.paperId && receipt.signerPublicKey === input.signerPublicKey) {
      return receipt;
    }
    throw this.alreadyUsed();
  }

  /** Devuelve la hora del ledger (ISO) del pago verificado. */
  private async verifyOnHorizon(input: VerifiedPayment): Promise<string> {
    const transaction = await this.fetchJson<HorizonTransaction>(`/transactions/${input.txHash}`);
    if (transaction.successful !== true) {
      throw new PaymentVerificationError(402, 'La transacción no fue confirmada correctamente en Stellar.');
    }
    if (transaction.source_account !== input.signerPublicKey) {
      throw new PaymentVerificationError(402, 'El firmante no coincide con el origen de la transacción.');
    }
    this.requireRecent(transaction.created_at);

    const operations = await this.fetchJson<HorizonCollection<HorizonPaymentOperation>>(
      `/transactions/${input.txHash}/operations?limit=200&order=asc`,
    );
    const hasExactPayment = (operations._embedded?.records ?? []).some((operation) => (
      operation.type === 'payment' &&
      operation.from === input.signerPublicKey &&
      operation.to === config.stellarTreasuryPublicKey &&
      operation.asset_type === 'credit_alphanum4' &&
      operation.asset_code === 'USDC' &&
      operation.asset_issuer === USDC_TESTNET_ISSUER &&
      this.isExactPrice(operation.amount)
    ));

    if (!hasExactPayment) {
      throw new PaymentVerificationError(402, 'La transacción no contiene un pago exacto de 0.50 USDC a la tesorería.');
    }
    return new Date(Date.parse(transaction.created_at as string)).toISOString();
  }

  private requireRecent(createdAt: string | undefined): void {
    const timestamp = Date.parse(createdAt ?? '');
    const maxAgeMilliseconds = config.paymentVerificationMaxAgeSeconds * 1000;
    const age = Date.now() - timestamp;
    if (!Number.isFinite(timestamp) || age < -60_000 || age > maxAgeMilliseconds) {
      throw new PaymentVerificationError(402, 'La transacción no está dentro del periodo de pago permitido.');
    }
  }

  private isExactPrice(amount: string | undefined): boolean {
    if (!amount || !/^\d+(?:\.\d{1,7})?$/.test(amount)) return false;
    const [whole, fraction = ''] = amount.split('.');
    const stroops = BigInt(`${whole}${fraction.padEnd(7, '0')}`);
    return stroops === BigInt(DEFAULT_PAPER_PRICE_STROOPS);
  }

  private async fetchJson<T>(path: string): Promise<T> {
    let response: Response;
    try {
      response = await fetch(`${config.horizonUrl}${path}`, { signal: AbortSignal.timeout(8000) });
    } catch {
      throw new PaymentVerificationError(502, 'No fue posible consultar Horizon (timeout o red).');
    }
    if (response.status === 404) {
      throw new PaymentVerificationError(402, 'No se encontró la transacción en Horizon.');
    }
    if (!response.ok) {
      throw new PaymentVerificationError(502, 'Horizon rechazó la consulta de verificación.');
    }
    try {
      return await response.json() as T;
    } catch {
      throw new PaymentVerificationError(502, 'Horizon devolvió una respuesta inválida.');
    }
  }
}

export const paymentVerificationService = new PaymentVerificationService();
