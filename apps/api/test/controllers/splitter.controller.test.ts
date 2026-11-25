import { afterEach, describe, expect, it, vi } from 'vitest';
import request from 'supertest';
import { Keypair, Networks, Transaction, TransactionBuilder, Account, BASE_FEE, Operation, Asset } from '@stellar/stellar-sdk';
import app from '../../src/app';
import { SplitterError, splitterService } from '../../src/services/splitter.service';
import { receiptStore } from '../../src/services/receipt.store';
import { decodeBase64Json, encodeBase64Json } from '../../src/utils/base64';
import { jwtService } from '../../src/services/jwt.service';
import { SplitterPrepareResponse, X402PaymentRequiredHeader, X402PaymentSignatureHeader } from '@paperpay/shared';

const PAPER = 'autonomous-ai-micropayments';
const CONTRACT = 'CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5';
const PUBLISHER = 'GA3MOPOJ374G4APKR4RE7VPXF5CEQKER3FA3LZ2XSAARIRSZEF6QXUZI';
const reader = Keypair.random();

function enableSplitter() {
  vi.spyOn(splitterService, 'enabled', 'get').mockReturnValue(true);
  vi.spyOn(splitterService, 'terms').mockReturnValue({ contractId: CONTRACT, publisher: PUBLISHER, amount: '5000000' });
}

function signatureHeader(signature: string, claim?: string): string {
  const payload: X402PaymentSignatureHeader = {
    scheme: 'exact',
    network: 'stellar:testnet',
    signature,
    signerPublicKey: reader.publicKey(),
    ...(claim && { claim }),
  };
  return encodeBase64Json(payload);
}

function anySignedXdr(): string {
  const tx = new TransactionBuilder(new Account(reader.publicKey(), '1'), { fee: BASE_FEE, networkPassphrase: Networks.TESTNET })
    .addOperation(Operation.payment({ destination: PUBLISHER, asset: Asset.native(), amount: '1' }))
    .setTimeout(60)
    .build();
  tx.sign(reader);
  return tx.toXDR();
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('Pagos por el contrato de reparto', () => {
  it('sin contrato configurado, el 402 y el health no lo anuncian', async () => {
    const res = await request(app).get(`/api/papers/${PAPER}`);
    expect(res.status).toBe(402);
    const header = decodeBase64Json<X402PaymentRequiredHeader>(res.headers['payment-required']);
    expect(header.accepts[0].extra?.splitter).toBeUndefined();

    const health = await request(app).get('/api/health');
    expect(health.body.splitterContractId).toBeNull();
  });

  it('con contrato, el 402 anuncia contrato, editorial y dónde preparar el pago', async () => {
    enableSplitter();
    const res = await request(app).get(`/api/papers/${PAPER}`);
    expect(res.status).toBe(402);
    const header = decodeBase64Json<X402PaymentRequiredHeader>(res.headers['payment-required']);
    expect(header.accepts[0].extra?.splitter).toEqual({
      contractId: CONTRACT,
      function: 'pay',
      publisher: PUBLISHER,
      prepareUrl: `/api/papers/${PAPER}/prepare`,
      giftUrl: `/api/papers/${PAPER}/gift`,
    });
    expect(res.body.paper).toBeUndefined();

    const health = await request(app).get('/api/health');
    expect(health.body.splitterContractId).toBe(CONTRACT);
  });

  describe('POST /api/papers/:id/prepare', () => {
    it('404 si el artículo no existe', async () => {
      enableSplitter();
      const res = await request(app).post('/api/papers/no-existe/prepare').send({ payer: reader.publicKey() });
      expect(res.status).toBe(404);
      expect(res.body.error).toBe('PAPER_NOT_FOUND');
    });

    it('404 SPLITTER_DISABLED si la API no cobra por contrato', async () => {
      const res = await request(app).post(`/api/papers/${PAPER}/prepare`).send({ payer: reader.publicKey() });
      expect(res.status).toBe(404);
      expect(res.body.error).toBe('SPLITTER_DISABLED');
    });

    it('400 sin payer', async () => {
      enableSplitter();
      const res = await request(app).post(`/api/papers/${PAPER}/prepare`).send({});
      expect(res.status).toBe(400);
    });

    it('200 con el XDR simulado, la red y los términos', async () => {
      enableSplitter();
      const prepare = vi.spyOn(splitterService, 'preparePayment').mockResolvedValueOnce({ transactionXdr: 'AAAA-xdr', claim: 'comprobante' });
      const res = await request(app).post(`/api/papers/${PAPER}/prepare`).send({ payer: reader.publicKey() });
      expect(res.status).toBe(200);
      const body = res.body as SplitterPrepareResponse;
      expect(body).toEqual({
        transactionXdr: 'AAAA-xdr',
        claim: 'comprobante',
        networkPassphrase: Networks.TESTNET,
        terms: { contractId: CONTRACT, publisher: PUBLISHER, amount: '5000000' },
      });
      expect(prepare).toHaveBeenCalledWith(PAPER, reader.publicKey(), undefined);
    });

    it('con reader prepara el regalo; reader que no es texto → 400', async () => {
      enableSplitter();
      const student = Keypair.random().publicKey();
      const prepare = vi.spyOn(splitterService, 'preparePayment').mockResolvedValueOnce({ transactionXdr: 'GIFT-xdr' });
      const res = await request(app).post(`/api/papers/${PAPER}/prepare`).send({ payer: reader.publicKey(), reader: student });
      expect(res.status).toBe(200);
      expect(prepare).toHaveBeenCalledWith(PAPER, reader.publicKey(), student);
      const bad = await request(app).post(`/api/papers/${PAPER}/prepare`).send({ payer: reader.publicKey(), reader: 42 });
      expect(bad.status).toBe(400);
    });

    it('propaga el estado y el mensaje de un error de preparación', async () => {
      enableSplitter();
      vi.spyOn(splitterService, 'preparePayment').mockRejectedValueOnce(new SplitterError(402, 'sin saldo'));
      const res = await request(app).post(`/api/papers/${PAPER}/prepare`).send({ payer: reader.publicKey() });
      expect(res.status).toBe(402);
      expect(res.body).toEqual({ error: 'PREPARE_FAILED', message: 'sin saldo' });
    });
  });

  describe('GET /api/papers/:id con PAYMENT-SIGNATURE de un pago por contrato', () => {
    it('lo cobra por el contrato y entrega el artículo con un JWT del lector', async () => {
      enableSplitter();
      const xdr = anySignedXdr();
      const parsed = TransactionBuilder.fromXDR(xdr, Networks.TESTNET) as Transaction;
      vi.spyOn(splitterService, 'isContractPayment').mockReturnValue(true);
      const validate = vi.spyOn(splitterService, 'validateSignedPayment').mockReturnValue(parsed);
      const settle = vi.spyOn(splitterService, 'settle').mockResolvedValue({
        txHash: 'e'.repeat(64),
        receipt: {
          contractId: CONTRACT,
          publisher: PUBLISHER,
          reader: reader.publicKey(),
          paperId: PAPER,
          payer: reader.publicKey(),
          amount: 5_000_000n,
          fee: 100_000n,
        },
        settledAt: '2026-11-24T21:56:42.000Z',
      });
      vi.spyOn(console, 'log').mockImplementation(() => {});

      const res = await request(app).get(`/api/papers/${PAPER}`).set('payment-signature', signatureHeader(xdr, 'comprobante'));

      expect(res.status).toBe(200);
      expect(res.body.paper.id).toBe(PAPER);
      expect(res.body.txHash).toBe('e'.repeat(64));
      expect(validate).toHaveBeenCalledWith(xdr, reader.publicKey(), PAPER);
      expect(settle).toHaveBeenCalledWith(parsed, reader.publicKey(), PAPER, 'comprobante');
      const claims = jwtService.verifyAccessToken(res.body.accessToken);
      expect(claims?.sub).toBe(reader.publicKey());
      expect(claims?.paperId).toBe(PAPER);

      // El recibo queda guardado y se puede consultar.
      const receipt = await request(app).get(`/api/receipts/${'e'.repeat(64)}`);
      expect(receipt.status).toBe(200);
      expect(receipt.body).toMatchObject({
        txHash: 'e'.repeat(64),
        paperId: PAPER,
        payer: reader.publicKey(),
        method: 'splitter',
        amountStroops: '5000000',
        settledAt: '2026-11-24T21:56:42.000Z',
        explorerUrl: `https://stellar.expert/explorer/testnet/tx/${'e'.repeat(64)}`,
      });
      expect(receipt.body.paperTitle).toEqual(expect.any(String));
    });

    it('si no se puede guardar el recibo, el lector igual recibe su artículo (el cobro ya ocurrió)', async () => {
      enableSplitter();
      const xdr = anySignedXdr();
      vi.spyOn(splitterService, 'isContractPayment').mockReturnValue(true);
      vi.spyOn(splitterService, 'validateSignedPayment').mockReturnValue(TransactionBuilder.fromXDR(xdr, Networks.TESTNET) as Transaction);
      vi.spyOn(splitterService, 'settle').mockResolvedValue({
        txHash: 'd'.repeat(64),
        receipt: {
          contractId: CONTRACT, publisher: PUBLISHER, reader: reader.publicKey(), paperId: PAPER,
          payer: reader.publicKey(), amount: 5_000_000n, fee: 100_000n,
        },
        settledAt: '2026-11-24T21:56:42.000Z',
      });
      vi.spyOn(receiptStore, 'record').mockImplementation(() => { throw new Error('disco lleno'); });
      vi.spyOn(console, 'log').mockImplementation(() => {});
      const logged = vi.spyOn(console, 'error').mockImplementation(() => {});

      const res = await request(app).get(`/api/papers/${PAPER}`).set('payment-signature', signatureHeader(xdr, 'comprobante'));

      expect(res.status).toBe(200);
      expect(res.body.txHash).toBe('d'.repeat(64));
      expect(logged).toHaveBeenCalledWith(expect.stringContaining('No se pudo guardar el recibo'), expect.any(Error));
    });

    it('504 PAYMENT_PENDING con el hash si la red no confirmó a tiempo', async () => {
      enableSplitter();
      const xdr = anySignedXdr();
      vi.spyOn(splitterService, 'isContractPayment').mockReturnValue(true);
      vi.spyOn(splitterService, 'validateSignedPayment').mockReturnValue(TransactionBuilder.fromXDR(xdr, Networks.TESTNET) as Transaction);
      vi.spyOn(splitterService, 'settle').mockRejectedValue(new SplitterError(504, 'pendiente', 'f'.repeat(64)));
      vi.spyOn(console, 'log').mockImplementation(() => {});
      vi.spyOn(console, 'error').mockImplementation(() => {});

      const res = await request(app).get(`/api/papers/${PAPER}`).set('payment-signature', signatureHeader(xdr));
      expect(res.status).toBe(504);
      expect(res.body).toEqual({ error: 'PAYMENT_PENDING', message: 'pendiente', txHash: 'f'.repeat(64) });
    });

    it('400 si lo firmado no es el pago esperado, sin enviarlo a la red', async () => {
      enableSplitter();
      vi.spyOn(splitterService, 'isContractPayment').mockReturnValue(true);
      vi.spyOn(splitterService, 'validateSignedPayment').mockImplementation(() => {
        throw new SplitterError(400, 'El pago es por otro artículo.');
      });
      const settle = vi.spyOn(splitterService, 'settle');
      vi.spyOn(console, 'log').mockImplementation(() => {});
      vi.spyOn(console, 'error').mockImplementation(() => {});

      const res = await request(app).get(`/api/papers/${PAPER}`).set('payment-signature', signatureHeader(anySignedXdr()));
      expect(res.status).toBe(400);
      expect(res.body.message).toBe('El pago es por otro artículo.');
      expect(settle).not.toHaveBeenCalled();
    });
  });
});
