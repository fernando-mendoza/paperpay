import { STELLAR_NETWORK } from './constants';

export interface PaperPreview {
  id: string;
  title: string;
  authors: string[];
  abstract: string;
  publishedDate: string;
  publisher: string;
  doi?: string;
  priceUsdc: number;
  previewSnippet: string;
  discipline?: string;
}

export interface PaperFull extends PaperPreview {
  fullContentMarkdown: string;
  pdfDownloadUrl?: string;
  references: string[];
}

export interface X402PaymentRequirement {
  scheme: 'exact';
  network: typeof STELLAR_NETWORK;
  asset: string;          // Contract address of USDC SAC
  amount: string;         // e.g. '5000000'
  payTo: string;          // Treasury public key
  maxTimeoutSeconds: number;
  extra?: {
    paperId: string;
    title: string;
    /** Presente si la API cobra a través del contrato de reparto 98/2. */
    splitter?: X402SplitterTerms;
  };
}

/**
 * Pago a través del contrato de reparto: el cliente pide a `prepareUrl` (POST { payer }) la
 * invocación `pay(payer, paperId, publisher, amount)` ya simulada, la firma con la wallet y la manda
 * como `signature` en PAYMENT-SIGNATURE, igual que el pago clásico.
 */
export interface X402SplitterTerms {
  contractId: string;
  function: 'pay';
  publisher: string;
  prepareUrl: string;
  /**
   * Regalo: `prepareUrl` con `{ payer, reader }` arma `gift(payer, reader, …)` y el sobre firmado se
   * manda aquí (POST con PAYMENT-SIGNATURE). Sin este campo, la API no acepta regalos.
   */
  giftUrl?: string;
}

/** `POST giftUrl`: el regalo quedó pagado. Quien paga no recibe el artículo. */
export interface GiftSentResponse {
  txHash: string;
  paperId: string;
  /** Cuenta que recibe el acceso: la llave del enlace de regalo. */
  reader: string;
  settledAt: string;
}

/**
 * `POST /api/gifts/claim`: quien abre el enlace prueba que tiene la llave de `reader` firmando
 * `giftClaimMessage(txHash, signedAt)` (ed25519, en base64). La firma vale `GIFT_CLAIM_WINDOW_SECONDS`.
 */
export interface GiftClaimRequest {
  txHash: string;
  reader: string;
  /** Segundos Unix. */
  signedAt: number;
  signature: string;
}

export interface GiftClaimResponse {
  paperId: string;
  paperTitle: string | null;
  accessToken: string;
  txHash: string;
  settledAt: string;
}

/** `GET /api/receipts/:txHash`: el recibo que la API guardó para ese pago. */
export interface PaymentReceiptResponse {
  txHash: string;
  paperId: string;
  paperTitle: string | null;
  payer: string;
  /** Quien recibe el acceso: igual a `payer`, salvo en un regalo. */
  reader: string;
  method: 'pollar' | 'splitter' | 'classic' | 'facilitator';
  /** Total pagado en stroops de USDC (7 decimales): '5000000' = 0.50 USDC. */
  amountStroops: string;
  /** Hora del ledger en que se confirmó. */
  settledAt: string;
  /** La prueba pública: la transacción en stellar.expert. */
  explorerUrl: string;
}

export interface SplitterPrepareResponse {
  transactionXdr: string;
  // Comprobante de quien pidió este sobre. Viaja de vuelta en `claim` de PAYMENT-SIGNATURE: con él,
  // reenviar el sobre ya confirmado da acceso hasta 24 h después (sin él, solo 5 min).
  claim?: string;
  // Quien recomendó, si el sobre es `pay_ref`. Ausente = `pay` (sin referido, o la API lo descartó
  // porque es la misma wallet o porque esa cuenta no puede cobrar su parte).
  referrer?: string;
  networkPassphrase: string;
  terms: { contractId: string; publisher: string; amount: string };
}

export interface X402PaymentRequiredHeader {
  accepts: X402PaymentRequirement[];
}

export interface X402PaymentSignatureHeader {
  scheme: 'exact';
  network: typeof STELLAR_NETWORK;
  signature: string;      // Facilitador: autorización; SELF_SETTLE: sobre de transacción XDR
  signerPublicKey: string;// G...
  claim?: string;         // Pago por contrato: el comprobante que devolvió /prepare
}

// Pago ya enviado por una wallet custodial, como Pollar. El backend verifica el
// hash en Horizon antes de conceder el acceso.
export interface X402PaymentVerificationHeader {
  scheme: 'exact';
  network: typeof STELLAR_NETWORK;
  signerPublicKey: string;
  txHash: string;
}

export interface X402PaymentResponseHeader {
  success: boolean;
  txHash: string;
  settledAt: string;
}

export interface JWTAccessTokenClaims {
  sub: string;            // Public key del lector
  paperId: string;
  txHash: string;
  iat?: number;
  exp?: number;
}

export interface HealthCheckResponse {
  status: 'ok';
  network: typeof STELLAR_NETWORK;
  mode: 'FACILITATOR' | 'SELF_SETTLE';
  treasuryPublicKey: string;
  /** Contrato de reparto 98/2 activo, o null si la API solo acepta pagos clásicos. */
  splitterContractId: string | null;
  /** El contrato activo y los anteriores (sus lecturas siguen contando). Vacío sin contrato. */
  splitterContractIds?: string[];
  timestamp: string;
}

export interface ApiErrorResponse {
  error: string;
  message: string;
}
