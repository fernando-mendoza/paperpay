import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { afterEach, describe, expect, it } from 'vitest';
import { ReceiptStore, type Receipt } from '../../src/services/receipt.store';

const receipt: Receipt = {
  txHash: 'ab'.repeat(32),
  paperId: 'autonomous-ai-micropayments',
  payer: 'GCLIENT',
  reader: 'GCLIENT',
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

  it('un regalo guarda a quien recibe el acceso; sin reader, el acceso es de quien paga', () => {
    const store = new ReceiptStore(':memory:');
    const { reader: _ignored, ...withoutReader } = receipt;
    expect(store.record(withoutReader).reader).toBe('GCLIENT');
    const gift = store.record({ ...receipt, txHash: 'cd'.repeat(32), method: 'splitter', reader: 'GESTUDIANTE' });
    expect(gift).toMatchObject({ payer: 'GCLIENT', reader: 'GESTUDIANTE' });
  });

  it('migra un archivo v1 (sin reader): los recibos anteriores quedan a nombre de quien pagó', () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'paperpay-recibos-v1-'));
    dirs.push(dir);
    const file = path.join(dir, 'receipts.db');
    const v1 = new DatabaseSync(file);
    v1.exec(`
      CREATE TABLE receipts (
        tx_hash TEXT PRIMARY KEY CHECK (length(tx_hash) = 64), paper_id TEXT NOT NULL, payer TEXT NOT NULL,
        method TEXT NOT NULL CHECK (method IN ('pollar', 'splitter', 'classic', 'facilitator')),
        amount_stroops TEXT NOT NULL, settled_at TEXT NOT NULL, recorded_at TEXT NOT NULL
      ) STRICT;
      PRAGMA user_version = 1;
    `);
    v1.prepare('INSERT INTO receipts VALUES (?, ?, ?, ?, ?, ?, ?)').run(
      receipt.txHash, receipt.paperId, receipt.payer, receipt.method, receipt.amountStroops, receipt.settledAt, receipt.settledAt,
    );
    v1.close();

    const store = new ReceiptStore(file);
    expect(store.get(receipt.txHash)).toEqual(receipt);
    store.close();
    const check = new DatabaseSync(file);
    expect((check.prepare('PRAGMA user_version').get() as { user_version: number }).user_version).toBe(2);
    check.close();
    // Abrirlo otra vez no vuelve a migrar.
    const again = new ReceiptStore(file);
    expect(again.count()).toBe(1);
    again.close();
  });

  it('la tabla rechaza hashes que no miden 64 y métodos desconocidos', () => {
    const store = new ReceiptStore(':memory:');
    expect(() => store.record({ ...receipt, txHash: 'abc' })).toThrow();
    expect(() => store.record({ ...receipt, method: 'tarjeta' as Receipt['method'] })).toThrow();
    expect(store.count()).toBe(0);
  });
});
