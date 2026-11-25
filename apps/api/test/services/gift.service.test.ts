import { Keypair } from '@stellar/stellar-sdk';
import { giftClaimMessage } from '@paperpay/shared';
import { describe, expect, it, vi } from 'vitest';
import { GiftError, GiftService } from '../../src/services/gift.service';
import { ReceiptStore } from '../../src/services/receipt.store';
import type { SplitterService } from '../../src/services/splitter.service';

const NOW = 1_764_000_000; // segundos Unix
const TX = 'cd'.repeat(32);
const PAPER = 'autonomous-ai-micropayments';
const payer = Keypair.random().publicKey();
const link = Keypair.random(); // la llave que viaja en el enlace de regalo

function setup(opts: { reader?: string; payerIsReader?: boolean; inStore?: boolean; findGift?: SplitterService['findGift'] } = {}) {
  const store = new ReceiptStore(':memory:');
  if (opts.inStore !== false) {
    const reader = opts.payerIsReader ? payer : (opts.reader ?? link.publicKey());
    store.record({ txHash: TX, paperId: PAPER, payer, reader, method: 'splitter', amountStroops: '5000000', settledAt: '2026-11-24T23:00:00.000Z' });
  }
  const findGift = vi.fn(opts.findGift ?? (async () => null));
  const splitter = { findGift } as unknown as SplitterService;
  return { store, findGift, service: new GiftService(splitter, store, () => NOW * 1000) };
}

function signed(key: Keypair = link, signedAt = NOW, txHash = TX) {
  return {
    txHash,
    reader: key.publicKey(),
    signedAt,
    // sign() devuelve un Uint8Array en este SDK: pasar por Buffer para tener base64.
    signature: Buffer.from(key.sign(Buffer.from(giftClaimMessage(txHash, signedAt)))).toString('base64'),
  };
}

async function claimError(service: GiftService, body: object): Promise<GiftError> {
  try {
    await service.claim(body);
  } catch (err) {
    expect(err).toBeInstanceOf(GiftError);
    return err as GiftError;
  }
  throw new Error('el reclamo debía fallar');
}

describe('GiftService.claim', () => {
  it('con la firma de la llave del enlace devuelve el recibo del regalo', async () => {
    const { service, findGift } = setup();
    await expect(service.claim(signed())).resolves.toMatchObject({ paperId: PAPER, payer, reader: link.publicKey() });
    expect(findGift).not.toHaveBeenCalled();
  });

  it('el hash en mayúsculas también sirve (la firma usa el hash normalizado)', async () => {
    const { service } = setup();
    const body = signed();
    await expect(service.claim({ ...body, txHash: TX.toUpperCase() })).resolves.toMatchObject({ paperId: PAPER });
  });

  it('403 si firma otra llave: conocer el hash y el reader (públicos en la red) no basta', async () => {
    const { service } = setup();
    const intruder = Keypair.random();
    const forged = { ...signed(intruder), reader: link.publicKey() };
    expect((await claimError(service, forged)).code).toBe('GIFT_CLAIM_FORBIDDEN');
    // Firma válida de otra llave, pero el regalo es para la del enlace.
    expect((await claimError(service, signed(intruder))).code).toBe('GIFT_CLAIM_FORBIDDEN');
  });

  it('400 si la firma tiene más de 5 minutos (o viene del futuro)', async () => {
    const { service } = setup();
    expect((await claimError(service, signed(link, NOW - 301))).code).toBe('GIFT_CLAIM_EXPIRED');
    expect((await claimError(service, signed(link, NOW + 301))).code).toBe('GIFT_CLAIM_EXPIRED');
    await expect(service.claim(signed(link, NOW - 299))).resolves.toBeDefined();
  });

  it('404 si el hash es de un pago normal: un pago no se reclama como regalo', async () => {
    const { service } = setup({ payerIsReader: true });
    expect((await claimError(service, signed())).code).toBe('GIFT_NOT_FOUND');
  });

  it('sin recibo en SQLite, busca el regalo en la red y lo guarda', async () => {
    const { service, store, findGift } = setup({
      inStore: false,
      findGift: async () => ({
        txHash: TX,
        settledAt: '2026-11-24T23:00:00.000Z',
        receipt: { contractId: 'C', publisher: 'G', reader: link.publicKey(), payer, paperId: PAPER, amount: 5_000_000n, fee: 100_000n },
      }),
    });
    await expect(service.claim(signed())).resolves.toMatchObject({ paperId: PAPER, reader: link.publicKey() });
    expect(findGift).toHaveBeenCalledWith(TX);
    expect(store.get(TX)).toMatchObject({ method: 'splitter', payer, reader: link.publicKey() });
  });

  it('404 si no está ni en SQLite ni en la red, o si el RPC falla', async () => {
    expect((await claimError(setup({ inStore: false }).service, signed())).code).toBe('GIFT_NOT_FOUND');
    const broken = setup({ inStore: false, findGift: async () => { throw new Error('rpc caído'); } });
    vi.spyOn(console, 'warn').mockImplementationOnce(() => {});
    expect((await claimError(broken.service, signed())).code).toBe('GIFT_NOT_FOUND');
  });

  it('400 con un cuerpo incompleto o mal formado', async () => {
    const { service } = setup();
    for (const body of [{}, { ...signed(), txHash: 'abc' }, { ...signed(), reader: 'GNOVALIDA' }, { ...signed(), signedAt: '1' }]) {
      expect((await claimError(service, body)).code).toBe('INVALID_GIFT_CLAIM');
    }
    expect((await claimError(service, { ...signed(), signature: 'corta' })).code).toBe('GIFT_CLAIM_FORBIDDEN');
  });
});
