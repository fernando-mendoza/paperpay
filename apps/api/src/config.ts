import dotenv from 'dotenv';
import path from 'path';

// Cargar .env si existe
dotenv.config({ path: path.resolve(process.cwd(), '.env') });

export interface AppConfig {
  host: string;
  port: number;
  nodeEnv: string;
  stellarTreasuryPublicKey: string;
  selfSettle: boolean;
  demoPayments: boolean;
  openZeppelinChannelsUrl: string;
  openZeppelinApiKey: string;
  horizonUrl: string;
  paymentVerificationMaxAgeSeconds: number;
  jwtSecret: string;
  corsOrigins: string[];
}

const defaultTreasuryKey = 'GB6X402TREASURYDEMOUNAM2026GOYAHACKPAPERPAYTESTNET';

export const config: AppConfig = {
  // Sin HOST escucha en todas las interfaces (PaaS, Docker). En una VPS detrás de un
  // reverse proxy se fija HOST=127.0.0.1 para que la API no quede expuesta directo.
  host: process.env.HOST || '0.0.0.0',
  port: parseInt(process.env.PORT || '4000', 10),
  nodeEnv: process.env.NODE_ENV || 'development',
  stellarTreasuryPublicKey: process.env.STELLAR_TREASURY_PUBLIC_KEY || defaultTreasuryKey,
  selfSettle: process.env.SELF_SETTLE === 'true',
  demoPayments: process.env.DEMO_PAYMENTS === 'true' && process.env.NODE_ENV !== 'production',
  openZeppelinChannelsUrl: process.env.OPENZEPPELIN_CHANNELS_URL || 'https://channels.openzeppelin.com/x402/testnet',
  openZeppelinApiKey: process.env.OPENZEPPELIN_API_KEY || 'demo_key',
  horizonUrl: process.env.STELLAR_HORIZON_URL || 'https://horizon-testnet.stellar.org',
  paymentVerificationMaxAgeSeconds: parseInt(process.env.PAYMENT_VERIFICATION_MAX_AGE_SECONDS || '300', 10),
  jwtSecret: process.env.JWT_SECRET || 'paperpay_hackathon_goya_2026_default_secret_key',
  corsOrigins: process.env.CORS_ORIGINS
    ? process.env.CORS_ORIGINS.split(',').map((s) => s.trim())
    : ['http://localhost:3000', 'http://localhost:4000', 'https://paperpay.vercel.app'],
};
