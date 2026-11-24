/**
 * E2E en testnet del pago por el contrato de reparto. SÍ mueve fondos de prueba: crea un lector
 * desechable (Friendbot + 1 USDC del DEX de testnet) y paga 0.50 USDC por un artículo.
 *
 *   SPLITTER_CONTRACT_ID=C... SPLITTER_PUBLISHER_ADDRESS=G... STELLAR_TREASURY_PUBLIC_KEY=G... \
 *   PORT=4010 HOST=127.0.0.1 pnpm --filter @paperpay/api test:splitter
 *
 * Levanta la API en este proceso y recorre el flujo del navegador: 402 → /prepare → firma →
 * PAYMENT-SIGNATURE → 200 con JWT. Después compara los saldos en Horizon (98% / 2%).
 *
 * Con LATE_RESEND_SECONDS=310 espera ese tiempo y reenvía el mismo sobre ya confirmado (fuera de la
 * ventana de 5 min): con el comprobante de /prepare debe dar 200; sin él (el sobre público que
 * cualquiera puede copiar de la red), 409.
 */
import { Asset, BASE_FEE, Horizon, Keypair, Networks, Operation, TransactionBuilder } from '@stellar/stellar-sdk';
import {
  SplitterPrepareResponse,
  USDC_TESTNET_ISSUER,
  X402PaymentRequiredHeader,
  X402PaymentSignatureHeader,
} from '@paperpay/shared';
import { server } from '../src/index';
import { config } from '../src/config';

const baseUrl = process.env.API_URL || `http://127.0.0.1:${config.port}`;
const paperId = process.env.PAPER_ID || 'crispr-nanomedicine-biotech';
const USDC = new Asset('USDC', USDC_TESTNET_ISSUER);
const horizon = new Horizon.Server(config.horizonUrl);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

async function usdcBalance(account: string): Promise<number> {
  const loaded = await horizon.loadAccount(account);
  const line = loaded.balances.find((b) => 'asset_code' in b && b.asset_code === 'USDC' && b.asset_issuer === USDC_TESTNET_ISSUER);
  return line ? Number(line.balance) : 0;
}

async function createReader(): Promise<Keypair> {
  const reader = Keypair.random();
  const friendbot = await fetch(`https://friendbot.stellar.org?addr=${encodeURIComponent(reader.publicKey())}`);
  assert(friendbot.ok, `Friendbot respondió ${friendbot.status}`);
  const account = await horizon.loadAccount(reader.publicKey());
  const tx = new TransactionBuilder(account, { fee: String(Number(BASE_FEE) * 10), networkPassphrase: Networks.TESTNET })
    .addOperation(Operation.changeTrust({ asset: USDC }))
    .addOperation(Operation.pathPaymentStrictReceive({
      sendAsset: Asset.native(),
      sendMax: '50',
      destination: reader.publicKey(),
      destAsset: USDC,
      destAmount: '1',
      path: [],
    }))
    .setTimeout(60)
    .build();
  tx.sign(reader);
  await horizon.submitTransaction(tx);
  return reader;
}

async function run() {
  assert(config.splitterContractId, 'Falta SPLITTER_CONTRACT_ID');
  const reader = await createReader();
  const publisher = config.splitterPublisher;
  const treasury = config.stellarTreasuryPublicKey;
  const before = { reader: await usdcBalance(reader.publicKey()), publisher: await usdcBalance(publisher), treasury: await usdcBalance(treasury) };

  const path = `${baseUrl}/api/papers/${encodeURIComponent(paperId)}`;
  const locked = await fetch(path);
  assert(locked.status === 402, `Se esperaba 402, llegó ${locked.status}`);
  const required = JSON.parse(Buffer.from(locked.headers.get('payment-required') ?? '', 'base64').toString()) as X402PaymentRequiredHeader;
  const splitter = required.accepts[0]?.extra?.splitter;
  assert(splitter && splitter.contractId === config.splitterContractId, 'El 402 no anuncia el contrato de reparto');

  const prepared = await fetch(`${baseUrl}${splitter.prepareUrl}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ payer: reader.publicKey() }),
  });
  const preparedText = await prepared.text();
  assert(prepared.status === 200, `/prepare respondió ${prepared.status}: ${preparedText}`);
  const { transactionXdr, networkPassphrase, claim } = JSON.parse(preparedText) as SplitterPrepareResponse;
  assert(claim, '/prepare no devolvió el comprobante');

  // Lo que haría Freighter: firmar el sobre ya simulado.
  const tx = TransactionBuilder.fromXDR(transactionXdr, networkPassphrase);
  tx.sign(reader);
  const signature: X402PaymentSignatureHeader = {
    scheme: 'exact',
    network: 'stellar:testnet',
    signature: tx.toXDR(),
    signerPublicKey: reader.publicKey(),
  };
  const encode = (h: X402PaymentSignatureHeader) => Buffer.from(JSON.stringify(h)).toString('base64');
  const header = encode({ ...signature, claim });
  const headerWithoutClaim = encode(signature);

  const started = Date.now();
  const paid = await fetch(path, { headers: { 'payment-signature': header } });
  const settleMs = Date.now() - started;
  const paidBody = (await paid.json()) as { paper?: { fullContentMarkdown?: string }; accessToken?: string; txHash?: string; message?: string };
  assert(paid.status === 200, `Pago respondió ${paid.status}: ${paidBody.message}`);
  assert(paidBody.paper?.fullContentMarkdown && paidBody.accessToken && paidBody.txHash, 'El 200 no trae artículo, JWT y hash');

  const withJwt = await fetch(path, { headers: { authorization: `Bearer ${paidBody.accessToken}` } });
  const replay = await fetch(path, { headers: { 'payment-signature': header } });
  const replayBody = (await replay.json()) as { message?: string };

  await sleep(3000); // Horizon ingiere el ledger un poco después que el RPC.
  const after = { reader: await usdcBalance(reader.publicKey()), publisher: await usdcBalance(publisher), treasury: await usdcBalance(treasury) };
  const delta = (k: keyof typeof before) => Number((after[k] - before[k]).toFixed(7));

  const ops = (await (await fetch(`${config.horizonUrl}/transactions/${paidBody.txHash}/operations`)).json()) as {
    _embedded: { records: Array<{ type: string; asset_balance_changes?: Array<{ to: string; amount: string }> }> };
  };
  const changes = ops._embedded.records[0]?.asset_balance_changes ?? [];

  const summary = {
    contract: config.splitterContractId,
    paperId,
    reader: reader.publicKey(),
    txHash: paidBody.txHash,
    settleMs,
    jwtReentry: withJwt.status,
    replayStatus: replay.status,
    replayMessage: replayBody.message,
    deltas: { reader: delta('reader'), publisher: delta('publisher'), treasury: delta('treasury') },
    horizonTransfers: changes.map((c) => ({ to: c.to, amount: c.amount })),
  };
  console.log(JSON.stringify(summary, null, 2));

  const lateSeconds = Number(process.env.LATE_RESEND_SECONDS || 0);
  let late: { withClaim: number; withoutClaim: number; withoutClaimMessage?: string } | null = null;
  if (lateSeconds > 0) {
    console.log(`Esperando ${lateSeconds} s para reenviar el mismo sobre fuera de la ventana de 5 min…`);
    await sleep(lateSeconds * 1000);
    const withClaim = await fetch(path, { headers: { 'payment-signature': header } });
    const withClaimBody = (await withClaim.json()) as { accessToken?: string; txHash?: string };
    const withoutClaim = await fetch(path, { headers: { 'payment-signature': headerWithoutClaim } });
    const withoutClaimBody = (await withoutClaim.json()) as { message?: string };
    late = { withClaim: withClaim.status, withoutClaim: withoutClaim.status, withoutClaimMessage: withoutClaimBody.message };
    console.log(JSON.stringify({ lateResend: { seconds: lateSeconds, ...late, txHash: withClaimBody.txHash } }, null, 2));
    assert(withClaimBody.txHash === paidBody.txHash && withClaimBody.accessToken, 'El reenvío tardío no devolvió el mismo pago con JWT');
  }

  assert(withJwt.status === 200, 'El JWT no reabre el artículo');
  assert(summary.deltas.reader === -0.5, `El lector debía pagar 0.50 y pagó ${-summary.deltas.reader}`);
  assert(summary.deltas.publisher === 0.49, `La editorial debía recibir 0.49 y recibió ${summary.deltas.publisher}`);
  assert(summary.deltas.treasury === 0.01, `La tesorería debía recibir 0.01 y recibió ${summary.deltas.treasury}`);
  if (late) {
    assert(late.withClaim === 200, `Con comprobante, el reenvío tardío debía dar 200 y dio ${late.withClaim}`);
    assert(late.withoutClaim === 409, `Sin comprobante, el reenvío tardío debía dar 409 y dio ${late.withoutClaim}`);
    console.log('✅ Reenvío tardío: con comprobante da acceso; el sobre público sin comprobante, no.');
  }
  console.log('✅ Pago por contrato verificado en testnet: 0.49 a la editorial, 0.01 a la tesorería, artículo entregado.');
}

run()
  .catch((error) => {
    console.error('❌', error instanceof Error ? error.message : error);
    process.exitCode = 1;
  })
  .finally(() => server.close());
