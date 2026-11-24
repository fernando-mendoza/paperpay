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
}

export interface SplitterPrepareResponse {
  transactionXdr: string;
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
  timestamp: string;
}

export interface ApiErrorResponse {
  error: string;
  message: string;
}
