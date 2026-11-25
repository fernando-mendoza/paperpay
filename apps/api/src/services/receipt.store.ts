import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { DatabaseSync, type StatementSync } from 'node:sqlite';
import { config } from '../config';

/** Cómo llegó el pago: Pollar (verificado por hash), contrato de reparto o transferencia clásica. */
export type ReceiptMethod = 'pollar' | 'splitter' | 'classic' | 'facilitator';

export interface Receipt {
  txHash: string;
  paperId: string;
  payer: string;
  /** Quien recibe el acceso: `payer`, salvo en un regalo (`gift` del contrato). */
  reader: string;
  method: ReceiptMethod;
  /** Monto total pagado, en stroops de USDC (7 decimales). */
  amountStroops: string;
  /** Hora del ledger en que se confirmó el pago (ISO 8601). */
  settledAt: string;
}

/** Lo que se guarda: sin `reader`, el acceso es de quien paga. */
export type NewReceipt = Omit<Receipt, 'reader'> & { reader?: string };

interface ReceiptRow {
  tx_hash: string;
  paper_id: string;
  payer: string;
  reader: string;
  method: ReceiptMethod;
  amount_stroops: string;
  settled_at: string;
}

const SCHEMA_VERSION = 2;

/**
 * Recibos de pago en SQLite (`node:sqlite`, sin servidor aparte). El hash de la transacción es
 * la clave primaria: un pago queda ligado a un solo artículo y una sola wallet, y eso sobrevive
 * a reinicios. La red sigue siendo la fuente de verdad; esto es el índice que la API consulta.
 */
export class ReceiptStore {
  private readonly db: DatabaseSync;
  private readonly selectStmt: StatementSync;
  private readonly insertStmt: StatementSync;
  private readonly countStmt: StatementSync;

  public constructor(public readonly location: string) {
    if (location !== ':memory:') mkdirSync(path.dirname(location), { recursive: true });
    this.db = new DatabaseSync(location);
    this.db.exec(`
      PRAGMA journal_mode = WAL;
      PRAGMA synchronous = NORMAL;
      PRAGMA busy_timeout = 5000;
      CREATE TABLE IF NOT EXISTS receipts (
        tx_hash        TEXT PRIMARY KEY CHECK (length(tx_hash) = 64),
        paper_id       TEXT NOT NULL,
        payer          TEXT NOT NULL,
        method         TEXT NOT NULL CHECK (method IN ('pollar', 'splitter', 'classic', 'facilitator')),
        amount_stroops TEXT NOT NULL,
        settled_at     TEXT NOT NULL,
        recorded_at    TEXT NOT NULL
      ) STRICT;
    `);
    this.migrate();
    this.selectStmt = this.db.prepare(
      'SELECT tx_hash, paper_id, payer, reader, method, amount_stroops, settled_at FROM receipts WHERE tx_hash = ?',
    );
    this.insertStmt = this.db.prepare(`
      INSERT INTO receipts (tx_hash, paper_id, payer, reader, method, amount_stroops, settled_at, recorded_at)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT (tx_hash) DO NOTHING
    `);
    this.countStmt = this.db.prepare('SELECT count(*) AS n FROM receipts');
  }

  public get(txHash: string): Receipt | undefined {
    const row = this.selectStmt.get(txHash.toLowerCase()) as ReceiptRow | undefined;
    return row && {
      txHash: row.tx_hash,
      paperId: row.paper_id,
      payer: row.payer,
      reader: row.reader,
      method: row.method,
      amountStroops: row.amount_stroops,
      settledAt: row.settled_at,
    };
  }

  /**
   * Guarda el recibo si el hash es nuevo y devuelve el que queda guardado. Si el hash ya existía,
   * NO lo pisa: el que llama compara artículo y wallet para detectar un intento de reutilizarlo.
   */
  public record(receipt: NewReceipt): Receipt {
    const txHash = receipt.txHash.toLowerCase();
    this.insertStmt.run(
      txHash,
      receipt.paperId,
      receipt.payer,
      receipt.reader ?? receipt.payer,
      receipt.method,
      receipt.amountStroops,
      receipt.settledAt,
      new Date().toISOString(),
    );
    return this.get(txHash) as Receipt;
  }

  /** v1 → v2: columna `reader`; los recibos anteriores son de quien pagó. */
  private migrate(): void {
    const columns = this.db.prepare('PRAGMA table_info(receipts)').all() as { name: string }[];
    if (!columns.some((c) => c.name === 'reader')) {
      this.db.exec(`
        BEGIN;
        ALTER TABLE receipts ADD COLUMN reader TEXT;
        UPDATE receipts SET reader = payer WHERE reader IS NULL;
        COMMIT;
      `);
    }
    this.db.exec(`PRAGMA user_version = ${SCHEMA_VERSION}`);
  }

  public count(): number {
    return Number((this.countStmt.get() as { n: number | bigint }).n);
  }

  public clearForTests(): void {
    this.db.exec('DELETE FROM receipts');
  }

  public close(): void {
    if (this.db.isOpen) this.db.close();
  }
}

/** Mismo hash → mismo artículo, misma wallet y mismo lector. */
export function sameReceipt(stored: Receipt, paperId: string, payer: string, reader = payer): boolean {
  return stored.paperId === paperId && stored.payer === payer && stored.reader === reader;
}

export const receiptStore = new ReceiptStore(config.receiptsDbPath);
