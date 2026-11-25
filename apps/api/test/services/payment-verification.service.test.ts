import { afterEach, describe, expect, it, vi } from 'vitest';
import { config } from '../../src/config';
import {
  PaymentVerificationError,
  PaymentVerificationService,
} from '../../src/services/payment-verification.service';
import { ReceiptStore } from '../../src/services/receipt.store';

const treasury = 'GTREASURY';
const signer = 'GCLIENT';
const paperId = 'autonomous-ai-micropayments';
const txHash = 'a'.repeat(64);

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });
}

function validOperations() {
  return {
    _embedded: {
      records: [{
        type: 'payment',
        from: signer,
        to: treasury,
        amount: '0.5000000',
        asset_type: 'credit_alphanum4',
        asset_code: 'USDC',
        asset_issuer: 'GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5',
      }],
    },
  };
}

describe('PaymentVerificationService', () => {
  afterEach(() => vi.restoreAllMocks());

  it('accepts one recent, successful exact USDC payment and allows an idempotent retry', async () => {
    const previousTreasury = config.stellarTreasuryPublicKey;
    config.stellarTreasuryPublicKey = treasury;
    const service = new PaymentVerificationService(new ReceiptStore(':memory:'));
    const fetchMock = vi.spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(json({ successful: true, created_at: new Date().toISOString(), source_account: signer }))
      .mockResolvedValueOnce(json(validOperations()));

    try {
      await expect(service.verifyByHash({ txHash, signerPublicKey: signer, paperId })).resolves.toEqual({
        txHash, signerPublicKey: signer, paperId,
      });
      await expect(service.verifyByHash({ txHash, signerPublicKey: signer, paperId })).resolves.toEqual({
        txHash, signerPublicKey: signer, paperId,
      });
      expect(fetchMock).toHaveBeenCalledTimes(2);
    } finally {
      config.stellarTreasuryPublicKey = previousTreasury;
    }
  });

  it('rejects a payment whose amount is not exactly 0.50 USDC', async () => {
    const previousTreasury = config.stellarTreasuryPublicKey;
    config.stellarTreasuryPublicKey = treasury;
    const service = new PaymentVerificationService(new ReceiptStore(':memory:'));
    const operations = validOperations();
    operations._embedded.records[0].amount = '0.5000001';
    vi.spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(json({ successful: true, created_at: new Date().toISOString(), source_account: signer }))
      .mockResolvedValueOnce(json(operations));

    try {
      await expect(service.verifyByHash({ txHash, signerPublicKey: signer, paperId }))
        .rejects.toMatchObject<Partial<PaymentVerificationError>>({ status: 402 });
    } finally {
      config.stellarTreasuryPublicKey = previousTreasury;
    }
  });

  it('does not let a verified hash unlock a different paper', async () => {
    const previousTreasury = config.stellarTreasuryPublicKey;
    config.stellarTreasuryPublicKey = treasury;
    const service = new PaymentVerificationService(new ReceiptStore(':memory:'));
    vi.spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(json({ successful: true, created_at: new Date().toISOString(), source_account: signer }))
      .mockResolvedValueOnce(json(validOperations()));

    try {
      await service.verifyByHash({ txHash, signerPublicKey: signer, paperId });
      await expect(service.verifyByHash({ txHash, signerPublicKey: signer, paperId: 'another-paper' }))
        .rejects.toMatchObject<Partial<PaymentVerificationError>>({ status: 409 });
    } finally {
      config.stellarTreasuryPublicKey = previousTreasury;
    }
  });

  it('returns a retryable 502 when Horizon times out', async () => {
    const service = new PaymentVerificationService(new ReceiptStore(':memory:'));
    const timeout = vi.spyOn(AbortSignal, 'timeout');
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockRejectedValueOnce(new DOMException('Timed out', 'TimeoutError'));

    await expect(service.verifyByHash({ txHash, signerPublicKey: signer, paperId }))
      .rejects.toMatchObject<Partial<PaymentVerificationError>>({ status: 502 });
    expect(timeout).toHaveBeenCalledWith(8000);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][1]?.signal).toBeInstanceOf(AbortSignal);
  });

  it('returns a retryable 502 when Horizon sends invalid JSON', async () => {
    const service = new PaymentVerificationService(new ReceiptStore(':memory:'));
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(new Response('not JSON', { status: 200 }));

    await expect(service.verifyByHash({ txHash, signerPublicKey: signer, paperId }))
      .rejects.toMatchObject<Partial<PaymentVerificationError>>({ status: 502 });
  });
});

describe('PaymentVerificationService con recibos en SQLite', () => {
  afterEach(() => vi.restoreAllMocks());

  function withTreasury<T>(fn: () => Promise<T>): Promise<T> {
    const previous = config.stellarTreasuryPublicKey;
    config.stellarTreasuryPublicKey = treasury;
    return fn().finally(() => { config.stellarTreasuryPublicKey = previous; });
  }

  it('guarda el recibo con la hora del ledger de Horizon', () => withTreasury(async () => {
    const store = new ReceiptStore(':memory:');
    const createdAt = new Date(Date.now() - 30_000).toISOString();
    vi.spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(json({ successful: true, created_at: createdAt, source_account: signer }))
      .mockResolvedValueOnce(json(validOperations()));

    await new PaymentVerificationService(store).verifyByHash({ txHash, signerPublicKey: signer, paperId });

    expect(store.get(txHash)).toEqual({
      txHash, paperId, payer: signer, reader: signer, method: 'pollar', amountStroops: '5000000', settledAt: createdAt,
    });
  }));

  it('después de un reinicio el mismo hash no desbloquea otro artículo, sin consultar Horizon', () => withTreasury(async () => {
    const store = new ReceiptStore(':memory:');
    vi.spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(json({ successful: true, created_at: new Date().toISOString(), source_account: signer }))
      .mockResolvedValueOnce(json(validOperations()));
    await new PaymentVerificationService(store).verifyByHash({ txHash, signerPublicKey: signer, paperId });

    const fetchAfterRestart = vi.spyOn(globalThis, 'fetch');
    fetchAfterRestart.mockClear();
    const restarted = new PaymentVerificationService(store);

    await expect(restarted.verifyByHash({ txHash, signerPublicKey: signer, paperId: 'otro-articulo' }))
      .rejects.toMatchObject({ status: 409 });
    await expect(restarted.verifyByHash({ txHash, signerPublicKey: signer, paperId }))
      .resolves.toEqual({ txHash, signerPublicKey: signer, paperId });
    expect(fetchAfterRestart).not.toHaveBeenCalled();
  }));

  it('un hash ya verificado no da acceso otra vez pasada su ventana de 5 minutos', async () => {
    const store = new ReceiptStore(':memory:');
    store.record({
      txHash, paperId, payer: signer, method: 'pollar', amountStroops: '5000000',
      settledAt: new Date(Date.now() - (config.paymentVerificationMaxAgeSeconds + 60) * 1000).toISOString(),
    });
    const fetchMock = vi.spyOn(globalThis, 'fetch');

    await expect(new PaymentVerificationService(store).verifyByHash({ txHash, signerPublicKey: signer, paperId }))
      .rejects.toMatchObject({ status: 409, message: expect.stringContaining('ventana') });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('un hash que llegó por el contrato o el pago clásico no sirve en /verify', async () => {
    const store = new ReceiptStore(':memory:');
    store.record({
      txHash, paperId, payer: signer, method: 'splitter', amountStroops: '5000000',
      settledAt: new Date().toISOString(),
    });

    await expect(new PaymentVerificationService(store).verifyByHash({ txHash, signerPublicKey: signer, paperId }))
      .rejects.toMatchObject({ status: 409 });
  });
});
