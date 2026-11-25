import { afterEach, describe, expect, it, vi } from 'vitest';
import request from 'supertest';
import { Account, BASE_FEE, Keypair, Networks, Operation, Asset, Transaction, TransactionBuilder } from '@stellar/stellar-sdk';
import { giftClaimMessage, type GiftClaimResponse, type GiftSentResponse, type X402PaymentSignatureHeader } from '@paperpay/shared';
import app from '../../src/app';
import { receiptStore } from '../../src/services/receipt.store';
import { SplitterError, splitterService } from '../../src/services/splitter.service';
import { encodeBase64Json } from '../../src/utils/base64';

const PAPER = 'autonomous-ai-micropayments';
const CONTRACT = 'CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5';
const PUBLISHER = 'GA3MOPOJ374G4APKR4RE7VPXF5CEQKER3FA3LZ2XSAARIRSZEF6QXUZI';
const TX = 'ef'.repeat(32);
const payer = Keypair.random();
const link = Keypair.random();

function enableSplitter() {
  vi.spyOn(splitterService, 'enabled', 'get').mockReturnValue(true);
  vi.spyOn(splitterService, 'terms').mockReturnValue({ contractId: CONTRACT, publisher: PUBLISHER, amount: '5000000' });
}

function giftHeader(): string {
  const tx = new TransactionBuilder(new Account(payer.publicKey(), '1'), { fee: BASE_FEE, networkPassphrase: Networks.TESTNET })
    .addOperation(Operation.payment({ destination: PUBLISHER, asset: Asset.native(), amount: '1' }))
    .setTimeout(60)
    .build();
  tx.sign(payer);
  const payload: X402PaymentSignatureHeader = { scheme: 'exact', network: 'stellar:testnet', signature: tx.toXDR(), signerPublicKey: payer.publicKey() };
  return encodeBase64Json(payload);
}

/** El contrato simulado: el sobre es un gift válido y la red lo confirma. */
function mockGiftSettles() {
  enableSplitter();
  vi.spyOn(splitterService, 'isContractPayment').mockReturnValue(true);
  const validate = vi.spyOn(splitterService, 'validateSignedGift').mockImplementation((xdrB64) => ({
    tx: TransactionBuilder.fromXDR(xdrB64, Networks.TESTNET) as Transaction,
    reader: link.publicKey(),
  }));
  const settle = vi.spyOn(splitterService, 'settleGift').mockResolvedValue({
    txHash: TX,
    settledAt: '2026-11-24T23:30:00.000Z',
    receipt: { contractId: CONTRACT, publisher: PUBLISHER, reader: link.publicKey(), payer: payer.publicKey(), paperId: PAPER, amount: 5_000_000n, fee: 100_000n },
  });
  return { validate, settle };
}

function claimBody(key = link, signedAt = Math.floor(Date.now() / 1000)) {
  return { txHash: TX, reader: key.publicKey(), signedAt, signature: Buffer.from(key.sign(Buffer.from(giftClaimMessage(TX, signedAt)))).toString('base64') };
}

afterEach(() => {
  vi.restoreAllMocks();
  receiptStore.clearForTests();
});

describe('POST /api/papers/:id/gift', () => {
  it('404 si el artículo no existe o la API no cobra por contrato', async () => {
    enableSplitter();
    expect((await request(app).post('/api/papers/no-existe/gift').set('payment-signature', giftHeader())).status).toBe(404);
    vi.restoreAllMocks();
    const res = await request(app).post(`/api/papers/${PAPER}/gift`).set('payment-signature', giftHeader());
    expect(res.status).toBe(404);
    expect(res.body.error).toBe('SPLITTER_DISABLED');
  });

  it('400 sin cabecera o con un sobre que no es de contrato', async () => {
    enableSplitter();
    expect((await request(app).post(`/api/papers/${PAPER}/gift`)).status).toBe(400);
    const res = await request(app).post(`/api/papers/${PAPER}/gift`).set('payment-signature', giftHeader());
    expect(res.status).toBe(400);
    expect(res.body.error).toBe('GIFT_FAILED');
  });

  it('200 con el recibo a nombre de quien recibe; quien paga no recibe el artículo ni un token', async () => {
    const { validate, settle } = mockGiftSettles();
    const res = await request(app).post(`/api/papers/${PAPER}/gift`).set('payment-signature', giftHeader());
    expect(res.status).toBe(200);
    expect(res.body as GiftSentResponse).toEqual({ txHash: TX, paperId: PAPER, reader: link.publicKey(), settledAt: '2026-11-24T23:30:00.000Z' });
    expect(res.body.paper).toBeUndefined();
    expect(res.body.accessToken).toBeUndefined();
    expect(validate).toHaveBeenCalledWith(expect.any(String), payer.publicKey(), PAPER);
    expect(settle).toHaveBeenCalledWith(expect.any(Transaction), payer.publicKey(), link.publicKey(), PAPER);

    const receipt = await request(app).get(`/api/receipts/${TX}`);
    expect(receipt.body).toMatchObject({ payer: payer.publicKey(), reader: link.publicKey(), method: 'splitter', paperId: PAPER });
  });

  it('504 si la red no confirma a tiempo (el sitio guarda el sobre y lo reenvía)', async () => {
    enableSplitter();
    vi.spyOn(splitterService, 'isContractPayment').mockReturnValue(true);
    vi.spyOn(splitterService, 'validateSignedGift').mockImplementation((xdrB64) => ({ tx: TransactionBuilder.fromXDR(xdrB64, Networks.TESTNET) as Transaction, reader: link.publicKey() }));
    vi.spyOn(splitterService, 'settleGift').mockRejectedValue(new SplitterError(504, 'pendiente', TX));
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const res = await request(app).post(`/api/papers/${PAPER}/gift`).set('payment-signature', giftHeader());
    expect(res.status).toBe(504);
    expect(res.body).toMatchObject({ error: 'PAYMENT_PENDING', txHash: TX });
  });
});

describe('POST /api/gifts/claim', () => {
  it('quien abre el enlace recibe un token que abre el artículo; el enlace sirve otra vez', async () => {
    mockGiftSettles();
    await request(app).post(`/api/papers/${PAPER}/gift`).set('payment-signature', giftHeader());

    const res = await request(app).post('/api/gifts/claim').send(claimBody());
    expect(res.status).toBe(200);
    const body = res.body as GiftClaimResponse;
    expect(body).toMatchObject({ paperId: PAPER, txHash: TX, settledAt: '2026-11-24T23:30:00.000Z' });
    expect(body.paperTitle).toBeTruthy();

    const paper = await request(app).get(`/api/papers/${PAPER}`).set('Authorization', `Bearer ${body.accessToken}`);
    expect(paper.status).toBe(200);
    expect(paper.body.paper.fullContentMarkdown.length).toBeGreaterThan(100);

    expect((await request(app).post('/api/gifts/claim').send(claimBody())).status).toBe(200);
  });

  it('403 con otra llave y 400 con una firma vencida', async () => {
    mockGiftSettles();
    await request(app).post(`/api/papers/${PAPER}/gift`).set('payment-signature', giftHeader());
    const forbidden = await request(app).post('/api/gifts/claim').send(claimBody(Keypair.random()));
    expect(forbidden.status).toBe(403);
    expect(forbidden.body.error).toBe('GIFT_CLAIM_FORBIDDEN');
    const expired = await request(app).post('/api/gifts/claim').send(claimBody(link, Math.floor(Date.now() / 1000) - 600));
    expect(expired.status).toBe(400);
    expect(expired.body.error).toBe('GIFT_CLAIM_EXPIRED');
  });

  it('404 si no hay regalo con ese hash', async () => {
    vi.spyOn(splitterService, 'findGift').mockResolvedValue(null);
    const res = await request(app).post('/api/gifts/claim').send(claimBody());
    expect(res.status).toBe(404);
    expect(res.body.error).toBe('GIFT_NOT_FOUND');
  });
});
