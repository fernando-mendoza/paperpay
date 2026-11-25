import {
  DEFAULT_PAPER_PRICE_STROOPS,
  STELLAR_NETWORK,
  USDC_TESTNET_CONTRACT,
  X402PaymentRequiredHeader,
  X402PaymentSignatureHeader,
} from '@paperpay/shared';
import { config } from '../config';
import { decodeBase64Json } from '../utils/base64';
import { ReceiptStore, receiptStore, sameReceipt, type NewReceipt } from './receipt.store';
import { SPLITTER_PAY_FUNCTION, splitterService } from './splitter.service';

export class X402Service {
  public constructor(private readonly receipts: ReceiptStore = receiptStore) {}

  public createPaymentRequirement(paperId: string, title: string): X402PaymentRequiredHeader {
    return {
      accepts: [
        {
          scheme: 'exact',
          network: STELLAR_NETWORK,
          asset: USDC_TESTNET_CONTRACT,
          amount: DEFAULT_PAPER_PRICE_STROOPS, // '5000000' = 0.50 USDC
          payTo: config.stellarTreasuryPublicKey,
          maxTimeoutSeconds: 300,
          extra: {
            paperId,
            title,
            ...(splitterService.enabled && {
              splitter: {
                contractId: splitterService.terms().contractId,
                function: SPLITTER_PAY_FUNCTION,
                publisher: splitterService.terms().publisher,
                prepareUrl: `/api/papers/${encodeURIComponent(paperId)}/prepare`,
                giftUrl: `/api/papers/${encodeURIComponent(paperId)}/gift`,
                ...(splitterService.referralsEnabled && { referrals: true }),
              },
            }),
          },
        },
      ],
    };
  }

  public parseSignatureHeader(rawHeader: string): X402PaymentSignatureHeader {
    try {
      // Intentar primero como Base64
      return decodeBase64Json<X402PaymentSignatureHeader>(rawHeader);
    } catch {
      try {
        // Intentar como JSON plano
        return JSON.parse(rawHeader) as X402PaymentSignatureHeader;
      } catch {
        throw new Error('Formato de cabecera PAYMENT-SIGNATURE inválido. Debe ser JSON o Base64.');
      }
    }
  }

  public async settlePayment(
    signaturePayload: X402PaymentSignatureHeader,
    paperId: string
  ): Promise<{ success: boolean; txHash: string }> {
    if (signaturePayload.scheme !== 'exact' || signaturePayload.network !== STELLAR_NETWORK ||
        !signaturePayload.signature || !signaturePayload.signerPublicKey) {
      throw new Error('Firma o clave pública del lector no proporcionada en la cabecera.');
    }

    const isPlaceholder = signaturePayload.signature === 'unsigned-demo-signature' ||
      signaturePayload.signature.startsWith('mock_') ||
      signaturePayload.signature.startsWith('demo_');
    if (isPlaceholder) {
      if (!config.demoPayments) throw new Error('Los pagos simulados están deshabilitados.');
      return { success: true, txHash: `mock_tx_${Date.now().toString(16)}_${Math.random().toString(16).slice(2, 10)}` };
    }

    console.log(`[x402Service] Iniciando liquidación para paper ${paperId} desde wallet ${signaturePayload.signerPublicKey}`);

    // Pago por el contrato de reparto 98/2: no depende del modo (lo envía la API, lo paga el lector).
    if (splitterService.enabled && splitterService.isContractPayment(signaturePayload.signature)) {
      const tx = splitterService.validateSignedPayment(signaturePayload.signature, signaturePayload.signerPublicKey, paperId);
      const { txHash, receipt, settledAt } = await splitterService.settle(tx, signaturePayload.signerPublicKey, paperId, signaturePayload.claim);
      this.recordReceipt({
        txHash, paperId, payer: signaturePayload.signerPublicKey, method: 'splitter',
        amountStroops: receipt.amount.toString(), settledAt,
      });
      return { success: true, txHash };
    }

    // Modo 1: SELF_SETTLE (Fallback con cuenta operativa del backend)
    if (config.selfSettle) {
      const settled = await this.selfSettle(signaturePayload, paperId);
      this.recordReceipt({
        txHash: settled.txHash, paperId, payer: signaturePayload.signerPublicKey, method: 'classic',
        amountStroops: String(DEFAULT_PAPER_PRICE_STROOPS), settledAt: settled.settledAt,
      });
      return { success: true, txHash: settled.txHash };
    }

    // Modo 2: Facilitador OpenZeppelin Channels
    try {
      const settled = await this.settleViaOpenZeppelin(signaturePayload, paperId);
      this.recordReceipt({
        txHash: settled.txHash, paperId, payer: signaturePayload.signerPublicKey, method: 'facilitator',
        amountStroops: String(DEFAULT_PAPER_PRICE_STROOPS), settledAt: new Date().toISOString(),
      });
      return settled;
    } catch (err: unknown) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      console.warn(`[x402Service] Facilitador de OpenZeppelin falló o no disponible: ${errorMsg}`);

      throw new Error(`Error en facilitador x402: ${errorMsg}`);
    }
  }

  private async settleViaOpenZeppelin(
    payload: X402PaymentSignatureHeader,
    paperId: string
  ): Promise<{ success: boolean; txHash: string }> {
    const response = await fetch(config.openZeppelinChannelsUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-API-Key': config.openZeppelinApiKey,
      },
      body: JSON.stringify({
        scheme: 'exact',
        network: STELLAR_NETWORK,
        signature: payload.signature,
        signer: payload.signerPublicKey,
        asset: USDC_TESTNET_CONTRACT,
        amount: DEFAULT_PAPER_PRICE_STROOPS,
        payTo: config.stellarTreasuryPublicKey,
        metadata: { paperId },
      }),
    });

    if (!response.ok) {
      const errorBody = await response.text();
      throw new Error(`OpenZeppelin Channels HTTP ${response.status}: ${errorBody}`);
    }

    const result = (await response.json()) as { success?: boolean; txHash?: string };
    if (result.success !== true || !result.txHash || !/^[0-9a-f]{64}$/i.test(result.txHash)) {
      throw new Error('El facilitador no confirmó una transacción válida.');
    }
    return {
      success: true,
      txHash: result.txHash,
    };
  }

  /**
   * Guarda el recibo del pago ya liquidado. El cobro ya ocurrió en la red: si la escritura falla,
   * se registra el error y el lector recibe su acceso igual.
   */
  private recordReceipt(receipt: NewReceipt): void {
    try {
      const saved = this.receipts.record(receipt);
      if (!sameReceipt(saved, receipt.paperId, receipt.payer)) {
        console.error(`[x402Service] El hash ${receipt.txHash} ya tenía recibo de otro artículo o wallet.`);
      }
    } catch (err: unknown) {
      console.error(`[x402Service] No se pudo guardar el recibo ${receipt.txHash}:`, err);
    }
  }

  private async selfSettle(
    payload: X402PaymentSignatureHeader,
    _paperId: string
  ): Promise<{ txHash: string; settledAt: string }> {
    console.log('[x402Service] Evaluando liquidación en modo SELF_SETTLE...');
    
    const { Transaction, Networks, Horizon } = await import('@stellar/stellar-sdk');
    
    let tx;
    try {
      tx = new Transaction(payload.signature, Networks.TESTNET);
    } catch {
      throw new Error('El campo signature no es un XDR de transacción válido.');
    }
    if (tx.source !== payload.signerPublicKey) {
      throw new Error('La wallet firmante no coincide con el origen de la transacción.');
    }

    // Validación de seguridad para MVP: asegurar que la transacción paga a nuestra tesorería, en USDC, y el monto es 0.50
    let isValidPayment = false;
    const USDC_ISSUER = 'GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5';

    for (const op of tx.operations) {
      if (op.type === 'payment' && (!op.source || op.source === payload.signerPublicKey) &&
          op.destination === config.stellarTreasuryPublicKey) {
        // En stellar-sdk, amount viene como string decimal (ej. "0.5000000")
        const amount = Number(op.amount);
        const isUSDC = op.asset && !op.asset.isNative() && op.asset.code === 'USDC' && op.asset.issuer === USDC_ISSUER;
        
        if (isUSDC && amount >= 0.5) {
          isValidPayment = true;
          break;
        }
      }
    }

    if (!isValidPayment) {
      throw new Error('La transacción no contiene un pago válido (0.50 USDC) hacia la tesorería de PaperPay.');
    }

    const server = new Horizon.Server('https://horizon-testnet.stellar.org');
    
    try {
      const response = await server.submitTransaction(tx);
      console.log(`✅ Transacción confirmada en Testnet! Hash: ${response.hash}`);
      return {
        txHash: response.hash,
        // Horizon responde cuando el ledger ya cerró: la hora de la respuesta es la del pago.
        settledAt: new Date().toISOString(),
      };
    } catch (err: any) {
      console.error('❌ Error enviando transacción a Horizon:', err.response?.data || err.message);
      throw new Error('Fallo al liquidar la transacción en la red Stellar.');
    }
  }
}

export const x402Service = new X402Service();
