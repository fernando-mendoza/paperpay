import app from './app';
import { config } from './config';
import { StrKey } from '@stellar/stellar-sdk';

if (config.nodeEnv === 'production' &&
    (config.jwtSecret.length < 32 || !StrKey.isValidEd25519PublicKey(config.stellarTreasuryPublicKey) ||
     (!config.selfSettle && !process.env.OPENZEPPELIN_API_KEY))) {
  throw new Error('Producción requiere JWT_SECRET seguro, tesorería Stellar válida y API key del facilitador si aplica.');
}

// Pagos por contrato: si se configura el contrato, la editorial que cobra tiene que ser válida.
if (config.splitterContractId &&
    (!StrKey.isValidContract(config.splitterContractId) || !StrKey.isValidEd25519PublicKey(config.splitterPublisher))) {
  throw new Error('SPLITTER_CONTRACT_ID debe ser un contrato (C...) y SPLITTER_PUBLISHER_ADDRESS una cuenta (G...).');
}

export const server = app.listen(config.port, config.host, () => {
  console.log(`PaperPay API escuchando en ${config.host}:${config.port}; modo ${config.selfSettle ? 'SELF_SETTLE' : 'FACILITATOR'}; reparto por contrato ${config.splitterContractId || 'deshabilitado'}.`);
});

// systemd (y la mayoría de los orquestadores) detienen con SIGTERM: se cierran las
// conexiones abiertas antes de salir para no cortar una verificación a la mitad.
for (const signal of ['SIGTERM', 'SIGINT'] as const) {
  process.once(signal, () => {
    console.log(`PaperPay API: ${signal} recibido, cerrando.`);
    server.close(() => process.exit(0));
    setTimeout(() => process.exit(1), 10_000).unref();
  });
}
