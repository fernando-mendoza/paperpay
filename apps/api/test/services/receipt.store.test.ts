import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { ReceiptStore, type Receipt } from '../../src/services/receipt.store';

const receipt: Receipt = {
  txHash: 'ab'.repeat(32),
  paperId: 'autonomous-ai-micropayments',
  payer: 'GCLIENT',
  method: 'pollar',
  amountStroops: '5000000',
  settledAt: '2026-11-24T21:56:42.000Z',
};

describe('ReceiptStore', () => {
  const dirs: string[] = [];
  afterEach(() => dirs.splice(0).forEach((dir) => rmSync(dir, { recursive: true, force: true })));

  it('guarda y lee un recibo; el hash se normaliza a minúsculas', () => {
    const store = new ReceiptStore(':memory:');
    store.record({ ...receipt, txHash: receipt.txHash.toUpperCase() });
    expect(store.get(receipt.txHash.toUpperCase())).toEqual(receipt);
    expect(store.count()).toBe(1);
  });

  it('no pisa un recibo existente: el mismo hash para otro artículo devuelve el original', () => {
    const store = new ReceiptStore(':memory:');
    store.record(receipt);
    const second = store.record({ ...receipt, paperId: 'otro-articulo', payer: 'GOTRO' });
    expect(second).toEqual(receipt);
    expect(store.count()).toBe(1);
  });

  it('sobrevive a cerrar y reabrir el archivo (un reinicio de la API)', () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'paperpay-recibos-'));
    dirs.push(dir);
    const file = path.join(dir, 'sub', 'receipts.db');

    const before = new ReceiptStore(file);
    before.record(receipt);
    before.close();

    const after = new ReceiptStore(file);
    expect(after.get(receipt.txHash)).toEqual(receipt);
    after.close();
  });

  it('la tabla rechaza hashes que no miden 64 y métodos desconocidos', () => {
    const store = new ReceiptStore(':memory:');
    expect(() => store.record({ ...receipt, txHash: 'abc' })).toThrow();
    expect(() => store.record({ ...receipt, method: 'tarjeta' as Receipt['method'] })).toThrow();
    expect(store.count()).toBe(0);
  });
});
