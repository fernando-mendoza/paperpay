import { describe, expect, it, vi } from 'vitest';
import {
  Account,
  Asset,
  BASE_FEE,
  Contract,
  Keypair,
  Networks,
  Operation,
  Transaction,
  TransactionBuilder,
  nativeToScVal,
  rpc,
  xdr,
} from '@stellar/stellar-sdk';
import { parsePaidReceipts, SplitterError, SplitterRpc, SplitterService } from '../../src/services/splitter.service';
import fixtures from '../fixtures/splitter-testnet-events.json';

// Contrato v0.2.0 en testnet y la editorial demo (contracts/deployments/testnet.json).
const CONTRACT = 'CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5';
const PUBLISHER = 'GA3MOPOJ374G4APKR4RE7VPXF5CEQKER3FA3LZ2XSAARIRSZEF6QXUZI';
const PAPER = 'crispr-nanomedicine-biotech';
const PRICE = 5_000_000n;
// Lector de los pagos reales de las fixtures (pay_batch y gift).
const FIXTURE_PAYER = 'GDAORJSX3SR6HM5B4HGXJGJGBEZMNLCMWR5WI7EZDXX6YH2HR2EPQLDY';
const FIXTURE_STUDENT = 'GCA6JEMN7Z4XVPXG6IEA5H7VKMUTQUBTH4SB2Z7A445UZIFDIHCLH2J6';

function fakeRpc(overrides: Partial<Record<keyof SplitterRpc, unknown>> = {}): SplitterRpc {
  return {
    getAccount: vi.fn(),
    prepareTransaction: vi.fn(),
    getTransaction: vi.fn().mockResolvedValue({ status: rpc.Api.GetTransactionStatus.NOT_FOUND }),
    sendTransaction: vi.fn().mockResolvedValue({ status: 'PENDING', hash: fixtures.pay_batch.txHash }),
    pollTransaction: vi.fn().mockResolvedValue(confirmed()),
    ...overrides,
  } as unknown as SplitterRpc;
}

// Momento de confirmación simulado (unix, segundos) para probar la ventana de recuperación.
const CONFIRMED_AT = 1_763_947_400;

function confirmed(fixture: { txHash: string; contractEventsXdr: string[][] } = fixtures.pay_batch) {
  return {
    status: rpc.Api.GetTransactionStatus.SUCCESS,
    txHash: fixture.txHash,
    createdAt: CONFIRMED_AT,
    events: { transactionEventsXdr: [], contractEventsXdr: [decode(fixture.contractEventsXdr[0])] },
  };
}

function service(client: SplitterRpc = fakeRpc(), nowSeconds = CONFIRMED_AT + 10) {
  return new SplitterService({
    contractId: CONTRACT,
    publisher: PUBLISHER,
    rpcUrl: 'http://rpc.invalid',
    networkPassphrase: Networks.TESTNET,
    rpc: client,
    now: () => nowSeconds * 1000,
    claimSecret: 'clave-de-prueba',
    referrals: true,
  });
}

function decode(events: string[]): xdr.ContractEvent[] {
  return events.map((b64) => xdr.ContractEvent.fromXDR(Buffer.from(b64, 'base64')));
}

function payArgs(payer: string, paper = PAPER, publisher = PUBLISHER, amount = PRICE): xdr.ScVal[] {
  return [
    nativeToScVal(payer, { type: 'address' }),
    nativeToScVal(paper, { type: 'string' }),
    nativeToScVal(publisher, { type: 'address' }),
    nativeToScVal(amount, { type: 'i128' }),
  ];
}

interface BuildOptions {
  contract?: string;
  fn?: string;
  args?: xdr.ScVal[];
  source?: string;
  signer?: Keypair | null;
  extraOp?: boolean;
  auth?: xdr.SorobanAuthorizationEntry[];
}

const reader = Keypair.random();

function build(opts: BuildOptions = {}): string {
  const source = opts.source ?? reader.publicKey();
  const call = new Contract(opts.contract ?? CONTRACT).call(opts.fn ?? 'pay', ...(opts.args ?? payArgs(reader.publicKey())));
  let op = call;
  if (opts.auth) {
    const parsed = new TransactionBuilder(new Account(source, '1'), { fee: BASE_FEE, networkPassphrase: Networks.TESTNET })
      .addOperation(call)
      .setTimeout(60)
      .build();
    const func = (parsed.operations[0] as Operation.InvokeHostFunction).func;
    op = Operation.invokeHostFunction({ func, auth: opts.auth });
  }
  const builder = new TransactionBuilder(new Account(source, '100'), { fee: BASE_FEE, networkPassphrase: Networks.TESTNET })
    .addOperation(op);
  if (opts.extraOp) {
    builder.addOperation(Operation.payment({ destination: PUBLISHER, asset: Asset.native(), amount: '1' }));
  }
  const tx = builder.setTimeout(300).build();
  if (opts.signer !== null) tx.sign(opts.signer ?? reader);
  return tx.toXDR();
}

function authFor(contract: string, fn: string, args: xdr.ScVal[]): xdr.SorobanAuthorizationEntry {
  const parsed = TransactionBuilder.fromXDR(build({ contract, fn, args }), Networks.TESTNET) as Transaction;
  const op = parsed.operations[0] as Operation.InvokeHostFunction;
  if (op.func.type !== 'hostFunctionTypeInvokeContract') throw new Error('no es invokeContract');
  return new xdr.SorobanAuthorizationEntry({
    credentials: xdr.SorobanCredentials.sorobanCredentialsSourceAccount(),
    rootInvocation: new xdr.SorobanAuthorizedInvocation({
      function: xdr.SorobanAuthorizedFunction.sorobanAuthorizedFunctionTypeContractFn(op.func.invokeContract),
      subInvocations: [],
    }),
  });
}

function rejects(xdrB64: string, message: RegExp, signer = reader.publicKey(), paper = PAPER) {
  let caught: unknown;
  try {
    service().validateSignedPayment(xdrB64, signer, paper);
  } catch (err) {
    caught = err;
  }
  expect(caught).toBeInstanceOf(SplitterError);
  expect((caught as SplitterError).status).toBe(400);
  expect((caught as SplitterError).message).toMatch(message);
}

describe('SplitterService.validateSignedPayment', () => {
  it('acepta pay de nuestro contrato, por este artículo, firmada por el lector', () => {
    const tx = service().validateSignedPayment(build(), reader.publicKey(), PAPER);
    expect(tx.source).toBe(reader.publicKey());
  });

  it('acepta la autorización de la cuenta origen sobre la misma llamada (como la deja la simulación)', () => {
    const auth = authFor(CONTRACT, 'pay', payArgs(reader.publicKey()));
    expect(() => service().validateSignedPayment(build({ auth: [auth] }), reader.publicKey(), PAPER)).not.toThrow();
  });

  it('rechaza otro contrato', () => {
    rejects(build({ contract: 'CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA' }), /contrato de reparto/);
  });

  it('rechaza otra función del contrato (gift, pay_batch)', () => {
    rejects(build({ fn: 'gift' }), /Solo se acepta la función pay/);
  });

  it('rechaza un pago por otro artículo', () => {
    rejects(build({ args: payArgs(reader.publicKey(), 'quantum-cryptography-post-quantum') }), /otro artículo/);
  });

  it('rechaza un pago a otra editorial (por ejemplo, a sí mismo)', () => {
    rejects(build({ args: payArgs(reader.publicKey(), PAPER, reader.publicKey()) }), /editorial/);
  });

  it('rechaza un monto distinto del precio', () => {
    rejects(build({ args: payArgs(reader.publicKey(), PAPER, PUBLISHER, 1n) }), /monto/);
  });

  it('rechaza que el pago salga de otra cuenta', () => {
    rejects(build({ args: payArgs(Keypair.random().publicKey()) }), /wallet que firma/);
  });

  it('rechaza que el origen de la transacción no sea quien dice firmar', () => {
    rejects(build(), /origen/, Keypair.random().publicKey());
  });

  it('rechaza más de una operación', () => {
    rejects(build({ extraOp: true }), /una sola operación/);
  });

  it('rechaza una transacción sin firma o firmada por otra llave', () => {
    rejects(build({ signer: null }), /no está firmada/);
    rejects(build({ signer: Keypair.random() }), /no está firmada/);
  });

  it('rechaza una autorización para otra llamada', () => {
    // Una autorización tiene que ser la misma llamada que la operación, no solo una permitida.
    rejects(build({ auth: [authFor(CONTRACT, 'gift', payArgs(reader.publicKey()))] }), /La autorización no corresponde/);
    rejects(build({ auth: [authFor(CONTRACT, 'pay', payArgs(reader.publicKey(), 'otro-articulo'))] }), /La autorización no corresponde/);
  });

  it('rechaza XDR que no es una transacción', () => {
    rejects('no-es-xdr', /XDR/);
  });

  it('distingue un pago por contrato de uno clásico', () => {
    const classic = new TransactionBuilder(new Account(reader.publicKey(), '1'), { fee: BASE_FEE, networkPassphrase: Networks.TESTNET })
      .addOperation(Operation.payment({ destination: PUBLISHER, asset: Asset.native(), amount: '1' }))
      .setTimeout(60)
      .build()
      .toXDR();
    expect(service().isContractPayment(build())).toBe(true);
    expect(service().isContractPayment(classic)).toBe(false);
    expect(service().isContractPayment('basura')).toBe(false);
  });
});

describe('parsePaidReceipts (eventos reales de testnet)', () => {
  it('lee los 6 comprobantes de pay_batch e ignora las transferencias del token', () => {
    const receipts = parsePaidReceipts(decode(fixtures.pay_batch.contractEventsXdr[0]));
    expect(receipts).toHaveLength(6);
    expect(receipts.map((r) => r.paperId)).toEqual([
      'crispr-nanomedicine-biotech',
      'gravitational-lensing-dark-matter',
      'autonomous-ai-micropayments',
      'stablecoins-emerging-markets',
      'battery-solid-state-electrolyte',
      'quantum-cryptography-post-quantum',
    ]);
    expect(receipts[0]).toEqual({
      contractId: CONTRACT,
      publisher: PUBLISHER,
      reader: FIXTURE_PAYER,
      paperId: 'crispr-nanomedicine-biotech',
      payer: FIXTURE_PAYER,
      amount: PRICE,
      fee: 100_000n,
    });
  });

  it('en gift, el lector es otra cuenta que la que paga', () => {
    const [receipt] = parsePaidReceipts(decode(fixtures.gift.contractEventsXdr[0]));
    expect(receipt.payer).toBe(FIXTURE_PAYER);
    expect(receipt.reader).toBe(FIXTURE_STUDENT);
    expect(receipt.paperId).toBe('stablecoins-emerging-markets');
  });
});

describe('SplitterService.settle', () => {
  const tx = () => TransactionBuilder.fromXDR(build(), Networks.TESTNET) as Transaction;

  it('confirma cuando la red trae el comprobante del artículo para este lector', async () => {
    const client = fakeRpc();
    const result = await service(client).settle(tx(), FIXTURE_PAYER, PAPER);
    expect(result.txHash).toBe(fixtures.pay_batch.txHash);
    expect(result.receipt.paperId).toBe(PAPER);
    expect(result.settledAt).toBe(new Date(CONFIRMED_AT * 1000).toISOString());
    expect(client.pollTransaction).toHaveBeenCalledWith(fixtures.pay_batch.txHash, { attempts: 30 });
  });

  it('no da acceso si el comprobante es de otro lector (un regalo no lo cobra quien paga)', async () => {
    const client = fakeRpc({
      pollTransaction: vi.fn().mockResolvedValue(confirmed(fixtures.gift)),
    });
    await expect(service(client).settle(tx(), FIXTURE_PAYER, 'stablecoins-emerging-markets')).rejects.toMatchObject({
      status: 502,
    });
  });

  it('no da acceso si el comprobante es de otro artículo', async () => {
    await expect(service().settle(tx(), FIXTURE_PAYER, 'no-esta-en-el-paquete')).rejects.toMatchObject({ status: 502 });
  });

  it('402 si la red rechaza el envío', async () => {
    const client = fakeRpc({ sendTransaction: vi.fn().mockResolvedValue({ status: 'ERROR', hash: 'ab'.repeat(32) }) });
    await expect(service(client).settle(tx(), FIXTURE_PAYER, PAPER)).rejects.toMatchObject({ status: 402, txHash: 'ab'.repeat(32) });
    expect(client.pollTransaction).not.toHaveBeenCalled();
  });

  it('402 si la transacción falla en la red', async () => {
    const client = fakeRpc({ pollTransaction: vi.fn().mockResolvedValue({ status: rpc.Api.GetTransactionStatus.FAILED }) });
    await expect(service(client).settle(tx(), FIXTURE_PAYER, PAPER)).rejects.toMatchObject({ status: 402 });
  });

  it('504 con el hash si sigue pendiente, para que el lector revise antes de reintentar', async () => {
    const client = fakeRpc({ pollTransaction: vi.fn().mockResolvedValue({ status: rpc.Api.GetTransactionStatus.NOT_FOUND }) });
    await expect(service(client).settle(tx(), FIXTURE_PAYER, PAPER)).rejects.toMatchObject({
      status: 504,
      txHash: fixtures.pay_batch.txHash,
    });
  });

  it('si el mismo pago ya está confirmado y es reciente, da el acceso sin reenviarlo (respuesta perdida)', async () => {
    const client = fakeRpc({ getTransaction: vi.fn().mockResolvedValue(confirmed()) });
    const result = await service(client, CONFIRMED_AT + 60).settle(tx(), FIXTURE_PAYER, PAPER);
    expect(result.txHash).toBe(fixtures.pay_batch.txHash);
    expect(client.sendTransaction).not.toHaveBeenCalled();
  });

  it('409 si el mismo pago ya está confirmado y es viejo: el sobre es público y no abre el artículo a otros', async () => {
    const client = fakeRpc({ getTransaction: vi.fn().mockResolvedValue(confirmed()) });
    await expect(service(client, CONFIRMED_AT + 301).settle(tx(), FIXTURE_PAYER, PAPER)).rejects.toMatchObject({
      status: 409,
      txHash: fixtures.pay_batch.txHash,
    });
    expect(client.sendTransaction).not.toHaveBeenCalled();
  });

  describe('reenvío del mismo sobre después de 5 minutos', () => {
    // Pide /prepare con un RPC que devuelve este sobre sin firmar, como lo recibe el lector.
    async function claimOf(unsignedXdr: string, claimSecret = 'clave-de-prueba') {
      const client = fakeRpc({
        getAccount: vi.fn().mockResolvedValue(new Account(reader.publicKey(), '99')),
        prepareTransaction: vi.fn(async () => TransactionBuilder.fromXDR(unsignedXdr, Networks.TESTNET)),
      });
      const svc = new SplitterService({ contractId: CONTRACT, publisher: PUBLISHER, rpcUrl: 'http://rpc.invalid', networkPassphrase: Networks.TESTNET, rpc: client, claimSecret });
      return (await svc.preparePayment(PAPER, reader.publicKey())).claim;
    }
    const unsigned = () => build({ signer: null });
    const confirmedRpc = () => fakeRpc({ getTransaction: vi.fn().mockResolvedValue(confirmed()) });

    it('con el comprobante de /prepare da el acceso aunque hayan pasado horas', async () => {
      const client = confirmedRpc();
      const claim = await claimOf(unsigned());
      const result = await service(client, CONFIRMED_AT + 3 * 3600).settle(tx(), FIXTURE_PAYER, PAPER, claim);
      expect(result.txHash).toBe(fixtures.pay_batch.txHash);
      expect(client.sendTransaction).not.toHaveBeenCalled();
    });

    it('409 con el comprobante de otro sobre o uno inventado (el sobre público de la red no lo trae)', async () => {
      const client = confirmedRpc();
      const otherClaim = await claimOf(build({ signer: null, args: payArgs(reader.publicKey(), 'otro-articulo') }));
      await expect(service(client, CONFIRMED_AT + 3600).settle(tx(), FIXTURE_PAYER, PAPER, otherClaim)).rejects.toMatchObject({ status: 409 });
      await expect(service(client, CONFIRMED_AT + 3600).settle(tx(), FIXTURE_PAYER, PAPER, 'x'.repeat(43))).rejects.toMatchObject({ status: 409 });
      await expect(service(client, CONFIRMED_AT + 3600).settle(tx(), FIXTURE_PAYER, PAPER)).rejects.toMatchObject({ status: 409 });
    });

    it('409 con un comprobante hecho con otra llave (otro JWT_SECRET)', async () => {
      const claim = await claimOf(unsigned(), 'otra-llave');
      await expect(service(confirmedRpc(), CONFIRMED_AT + 3600).settle(tx(), FIXTURE_PAYER, PAPER, claim)).rejects.toMatchObject({ status: 409 });
    });

    it('409 si el pago tiene más de 24 horas, aunque traiga el comprobante', async () => {
      const claim = await claimOf(unsigned());
      await expect(service(confirmedRpc(), CONFIRMED_AT + 24 * 3600 + 1).settle(tx(), FIXTURE_PAYER, PAPER, claim)).rejects.toMatchObject({
        status: 409,
        message: expect.stringMatching(/24 horas/),
      });
    });

    it('el comprobante no salta las demás revisiones: sigue exigiendo el `paid` de este artículo y lector', async () => {
      const claim = await claimOf(unsigned());
      await expect(service(confirmedRpc(), CONFIRMED_AT + 3600).settle(tx(), FIXTURE_PAYER, 'no-esta-en-el-paquete', claim)).rejects.toMatchObject({ status: 502 });
    });
  });

  it('402 si ese mismo pago ya falló en la red', async () => {
    const client = fakeRpc({ getTransaction: vi.fn().mockResolvedValue({ status: rpc.Api.GetTransactionStatus.FAILED, txHash: 'ab'.repeat(32) }) });
    await expect(service(client).settle(tx(), FIXTURE_PAYER, PAPER)).rejects.toMatchObject({ status: 402 });
    expect(client.sendTransaction).not.toHaveBeenCalled();
  });

  it('503 si la red pide reintentar más tarde', async () => {
    const client = fakeRpc({ sendTransaction: vi.fn().mockResolvedValue({ status: 'TRY_AGAIN_LATER', hash: 'cd'.repeat(32) }) });
    await expect(service(client).settle(tx(), FIXTURE_PAYER, PAPER)).rejects.toMatchObject({ status: 503 });
  });
});

describe('SplitterService.preparePayment', () => {
  it('arma pay con la editorial y el precio de la API y la simula', async () => {
    const prepareTransaction = vi.fn(async (t: Transaction) => t);
    const client = fakeRpc({
      getAccount: vi.fn().mockResolvedValue(new Account(reader.publicKey(), '41')),
      prepareTransaction,
    });
    const prepared = await service(client).preparePayment(PAPER, reader.publicKey());

    expect(prepareTransaction).toHaveBeenCalledOnce();
    expect(prepared.claim).toMatch(/^[\w-]{43}$/);
    // Lo que se devuelve, firmado por el lector, pasa la validación de cobro.
    const tx = TransactionBuilder.fromXDR(prepared.transactionXdr, Networks.TESTNET) as Transaction;
    expect(tx.source).toBe(reader.publicKey());
    expect(tx.sequence).toBe('42');
    tx.sign(reader);
    expect(() => service().validateSignedPayment(tx.toXDR(), reader.publicKey(), PAPER)).not.toThrow();
  });

  it('dos pedidos iguales en el mismo segundo dan sobres y comprobantes distintos', async () => {
    const client = fakeRpc({
      getAccount: vi.fn(async () => new Account(reader.publicKey(), '41')),
      prepareTransaction: vi.fn(async (t: Transaction) => t),
    });
    const svc = service(client);
    const results = await Promise.all(Array.from({ length: 5 }, () => svc.preparePayment(PAPER, reader.publicKey())));
    // 1000 comisiones posibles: que los 5 coincidan es prácticamente imposible.
    expect(new Set(results.map((r) => r.claim)).size).toBeGreaterThan(1);
    for (const r of results) {
      const fee = Number((TransactionBuilder.fromXDR(r.transactionXdr, Networks.TESTNET) as Transaction).fee);
      expect(fee).toBeGreaterThanOrEqual(100);
      expect(fee).toBeLessThan(1100);
    }
  });

  it('sin llave configurada no entrega comprobante', async () => {
    const client = fakeRpc({
      getAccount: vi.fn().mockResolvedValue(new Account(reader.publicKey(), '41')),
      prepareTransaction: vi.fn(async (t: Transaction) => t),
    });
    const svc = new SplitterService({ contractId: CONTRACT, publisher: PUBLISHER, rpcUrl: 'http://rpc.invalid', networkPassphrase: Networks.TESTNET, rpc: client });
    expect((await svc.preparePayment(PAPER, reader.publicKey())).claim).toBeUndefined();
  });

  it('400 si la wallet no es una dirección G válida', async () => {
    await expect(service().preparePayment(PAPER, 'no-es-wallet')).rejects.toMatchObject({ status: 400 });
  });

  it('402 si la wallet no existe en la red', async () => {
    const client = fakeRpc({ getAccount: vi.fn().mockRejectedValue(new Error('Account not found')) });
    await expect(service(client).preparePayment(PAPER, reader.publicKey())).rejects.toMatchObject({ status: 402 });
  });

  it('402 si la simulación falla (sin USDC o sin trustline)', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const client = fakeRpc({
      getAccount: vi.fn().mockResolvedValue(new Account(reader.publicKey(), '1')),
      prepareTransaction: vi.fn().mockRejectedValue(new Error('HostError: Error(Contract, #10)')),
    });
    await expect(service(client).preparePayment(PAPER, reader.publicKey())).rejects.toMatchObject({ status: 402 });
    warn.mockRestore();
  });

  it('deshabilitado sin contrato configurado', async () => {
    const off = new SplitterService({ contractId: '', publisher: '', rpcUrl: 'http://rpc.invalid', networkPassphrase: Networks.TESTNET });
    expect(off.enabled).toBe(false);
    await expect(off.preparePayment(PAPER, reader.publicKey())).rejects.toMatchObject({ status: 400 });
  });
});

describe('Regalo (gift)', () => {
  const student = Keypair.random().publicKey();
  const giftArgs = (payer = reader.publicKey(), to = student, paper = PAPER): xdr.ScVal[] => {
    const [payerArg, ...rest] = payArgs(payer, paper);
    return [payerArg, nativeToScVal(to, { type: 'address' }), ...rest];
  };
  const giftFixturePaper = 'stablecoins-emerging-markets';

  it('/prepare con reader arma gift(payer, reader, …) y lo firmado pasa la validación del regalo', async () => {
    const client = fakeRpc({
      getAccount: vi.fn().mockResolvedValue(new Account(reader.publicKey(), '41')),
      prepareTransaction: vi.fn(async (t: Transaction) => t),
    });
    const prepared = await service(client).preparePayment(PAPER, reader.publicKey(), student);
    const tx = TransactionBuilder.fromXDR(prepared.transactionXdr, Networks.TESTNET) as Transaction;
    const op = tx.operations[0] as Operation.InvokeHostFunction;
    if (op.func.type !== 'hostFunctionTypeInvokeContract') throw new Error('no es invokeContract');
    expect(op.func.invokeContract.functionName.toString()).toBe('gift');
    tx.sign(reader);
    expect(service().validateSignedGift(tx.toXDR(), reader.publicKey(), PAPER).reader).toBe(student);
    // El mismo sobre no sirve como pago normal: el acceso no es de quien paga.
    expect(() => service().validateSignedPayment(tx.toXDR(), reader.publicKey(), PAPER)).toThrow(/Solo se acepta la función pay/);
  });

  it('/prepare rechaza regalarse a sí mismo o a una dirección inválida', async () => {
    await expect(service().preparePayment(PAPER, reader.publicKey(), reader.publicKey())).rejects.toMatchObject({ status: 400 });
    await expect(service().preparePayment(PAPER, reader.publicKey(), 'GNOVALIDA')).rejects.toMatchObject({ status: 400 });
  });

  it('validateSignedGift rechaza pay, un regalo a quien firma y argumentos de más o de menos', () => {
    const check = (xdrB64: string) => () => service().validateSignedGift(xdrB64, reader.publicKey(), PAPER);
    expect(check(build())).toThrow(/Solo se acepta la función gift/);
    expect(check(build({ fn: 'gift', args: giftArgs(reader.publicKey(), reader.publicKey()) }))).toThrow(/otra cuenta/);
    expect(check(build({ fn: 'gift', args: payArgs(reader.publicKey()) }))).toThrow(SplitterError);
    expect(check(build({ fn: 'gift', args: [...giftArgs(), nativeToScVal(1, { type: 'u32' })] }))).toThrow(SplitterError);
    expect(check(build({ fn: 'gift', args: giftArgs(reader.publicKey(), student, 'otro-articulo') }))).toThrow(/otro artículo/);
    expect(check(build({ fn: 'gift', args: giftArgs(), signer: Keypair.random() }))).toThrow(/no está firmada/);
  });

  it('settleGift confirma con el comprobante a nombre de quien recibe (tx real de testnet)', async () => {
    const client = fakeRpc({ sendTransaction: vi.fn().mockResolvedValue({ status: 'PENDING', hash: fixtures.gift.txHash }), pollTransaction: vi.fn().mockResolvedValue(confirmed(fixtures.gift)) });
    const tx = TransactionBuilder.fromXDR(build({ fn: 'gift', args: giftArgs() }), Networks.TESTNET) as Transaction;
    const result = await service(client).settleGift(tx, FIXTURE_PAYER, FIXTURE_STUDENT, giftFixturePaper);
    expect(result.receipt).toMatchObject({ payer: FIXTURE_PAYER, reader: FIXTURE_STUDENT, paperId: giftFixturePaper });
    // Otro lector en el comprobante = no es este regalo.
    await expect(service(client).settleGift(tx, FIXTURE_PAYER, student, giftFixturePaper)).rejects.toMatchObject({ status: 502 });
  });

  it('reenviar un regalo confirmado hace días devuelve su recibo sin reenviarlo (no da acceso a quien paga)', async () => {
    const client = fakeRpc({ getTransaction: vi.fn().mockResolvedValue(confirmed(fixtures.gift)) });
    const tx = TransactionBuilder.fromXDR(build({ fn: 'gift', args: giftArgs() }), Networks.TESTNET) as Transaction;
    const result = await service(client, CONFIRMED_AT + 3 * 86_400).settleGift(tx, FIXTURE_PAYER, FIXTURE_STUDENT, giftFixturePaper);
    expect(result.txHash).toBe(fixtures.gift.txHash);
    expect(client.sendTransaction).not.toHaveBeenCalled();
  });

  it('findGift encuentra un regalo en la red y no confunde un pago normal con uno', async () => {
    const gift = await service(fakeRpc({ getTransaction: vi.fn().mockResolvedValue(confirmed(fixtures.gift)) })).findGift(fixtures.gift.txHash);
    expect(gift?.receipt).toMatchObject({ payer: FIXTURE_PAYER, reader: FIXTURE_STUDENT });
    expect(await service(fakeRpc({ getTransaction: vi.fn().mockResolvedValue(confirmed()) })).findGift(fixtures.pay_batch.txHash)).toBeNull();
    expect(await service().findGift('ab'.repeat(32))).toBeNull();
  });
});

describe('Referido (pay_ref)', () => {
  // v0.4.0 en testnet: el primero con pay_ref (contracts/deployments/testnet.json).
  const V040 = 'CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA';
  const referrer = Keypair.random().publicKey();
  const refArgs = (ref = referrer, payer = reader.publicKey()): xdr.ScVal[] => [...payArgs(payer), nativeToScVal(ref, { type: 'address' })];
  const fnOf = (transactionXdr: string): string => {
    const op = (TransactionBuilder.fromXDR(transactionXdr, Networks.TESTNET) as Transaction).operations[0] as Operation.InvokeHostFunction;
    if (op.func.type !== 'hostFunctionTypeInvokeContract') throw new Error('no es invokeContract');
    return op.func.invokeContract.functionName.toString();
  };
  const preparing = (prepareTransaction: unknown = vi.fn(async (t: Transaction) => t)) =>
    fakeRpc({ getAccount: vi.fn().mockResolvedValue(new Account(reader.publicKey(), '41')), prepareTransaction });

  it('/prepare con referrer arma pay_ref(payer, paper, publisher, amount, referrer) y lo firmado pasa como pago', async () => {
    const prepared = await service(preparing()).preparePayment(PAPER, reader.publicKey(), undefined, referrer);
    expect(prepared.referrer).toBe(referrer);
    expect(fnOf(prepared.transactionXdr)).toBe('pay_ref');
    const tx = TransactionBuilder.fromXDR(prepared.transactionXdr, Networks.TESTNET) as Transaction;
    tx.sign(reader);
    expect(() => service().validateSignedPayment(tx.toXDR(), reader.publicKey(), PAPER)).not.toThrow();
    // No sirve como regalo.
    expect(() => service().validateSignedGift(tx.toXDR(), reader.publicKey(), PAPER)).toThrow(/Solo se acepta la función gift/);
  });

  it('sin SPLITTER_REFERRALS (contrato sin pay_ref) el referido se ignora y se arma pay', async () => {
    const svc = new SplitterService({ contractId: CONTRACT, publisher: PUBLISHER, rpcUrl: 'http://rpc.invalid', networkPassphrase: Networks.TESTNET, rpc: preparing() });
    expect(svc.referralsEnabled).toBe(false);
    const prepared = await svc.preparePayment(PAPER, reader.publicKey(), undefined, referrer);
    expect(prepared.referrer).toBeUndefined();
    expect(fnOf(prepared.transactionXdr)).toBe('pay');
    expect(service().referralsEnabled).toBe(true);
  });

  it('si quien recomienda es la misma wallet que paga, arma pay sin referido', async () => {
    const prepared = await service(preparing()).preparePayment(PAPER, reader.publicKey(), undefined, reader.publicKey());
    expect(prepared.referrer).toBeUndefined();
    expect(fnOf(prepared.transactionXdr)).toBe('pay');
  });

  it('si la simulación de pay_ref falla (quien recomienda no puede cobrar), arma pay: el referido nunca impide pagar', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const prepareTransaction = vi
      .fn()
      .mockRejectedValueOnce(new Error('HostError: Error(Contract, #13) trustline entry is missing'))
      .mockImplementationOnce(async (t: Transaction) => t);
    const prepared = await service(preparing(prepareTransaction)).preparePayment(PAPER, reader.publicKey(), undefined, referrer);
    expect(prepareTransaction).toHaveBeenCalledTimes(2);
    expect(prepared.referrer).toBeUndefined();
    expect(fnOf(prepared.transactionXdr)).toBe('pay');
    // El segundo sobre sale de la secuencia de la cuenta, no de la del primero.
    expect((TransactionBuilder.fromXDR(prepared.transactionXdr, Networks.TESTNET) as Transaction).sequence).toBe('42');
    warn.mockRestore();
  });

  it('402 si tampoco se puede armar pay (al lector le falta saldo)', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const prepareTransaction = vi.fn().mockRejectedValue(new Error('HostError: balance is not sufficient'));
    await expect(service(preparing(prepareTransaction)).preparePayment(PAPER, reader.publicKey(), undefined, referrer)).rejects.toMatchObject({ status: 402 });
    warn.mockRestore();
  });

  it('400 con un referido que no es una cuenta G o en un regalo', async () => {
    await expect(service().preparePayment(PAPER, reader.publicKey(), undefined, 'GNOVALIDA')).rejects.toMatchObject({ status: 400 });
    await expect(service().preparePayment(PAPER, reader.publicKey(), undefined, CONTRACT)).rejects.toMatchObject({ status: 400 });
    await expect(service().preparePayment(PAPER, reader.publicKey(), Keypair.random().publicKey(), referrer)).rejects.toMatchObject({ status: 400 });
  });

  it('validateSignedPayment rechaza pay_ref sin referido, con referido = quien firma o con argumentos de más', () => {
    const check = (xdrB64: string) => () => service().validateSignedPayment(xdrB64, reader.publicKey(), PAPER);
    expect(check(build({ fn: 'pay_ref', args: refArgs() }))).not.toThrow();
    expect(check(build({ fn: 'pay_ref', args: payArgs(reader.publicKey()) }))).toThrow(SplitterError);
    expect(check(build({ fn: 'pay_ref', args: refArgs(reader.publicKey()) }))).toThrow(/otra cuenta/);
    expect(check(build({ fn: 'pay_ref', args: [...refArgs(), nativeToScVal(1, { type: 'u32' })] }))).toThrow(SplitterError);
    expect(check(build({ fn: 'pay_ref', args: [...payArgs(reader.publicKey(), PAPER, PUBLISHER, PRICE - 1n), nativeToScVal(referrer, { type: 'address' })] }))).toThrow(/precio/);
  });

  it('el pay_ref real de testnet deja un solo paid (comisión completa, acceso de quien paga); referred no es comprobante', () => {
    const receipts = parsePaidReceipts(decode(fixtures.pay_ref.contractEventsXdr[0]));
    expect(receipts).toHaveLength(1);
    expect(receipts[0]).toMatchObject({ contractId: V040, publisher: PUBLISHER, paperId: 'autonomous-ai-micropayments', amount: PRICE, fee: 100_000n });
    expect(receipts[0].reader).toBe(receipts[0].payer);
  });

  it('settle confirma un pay_ref real con su comprobante', async () => {
    const [receipt] = parsePaidReceipts(decode(fixtures.pay_ref.contractEventsXdr[0]));
    const client = fakeRpc({
      sendTransaction: vi.fn().mockResolvedValue({ status: 'PENDING', hash: fixtures.pay_ref.txHash }),
      pollTransaction: vi.fn().mockResolvedValue(confirmed(fixtures.pay_ref)),
    });
    const svc = new SplitterService({ contractId: V040, publisher: PUBLISHER, rpcUrl: 'http://rpc.invalid', networkPassphrase: Networks.TESTNET, rpc: client });
    const tx = TransactionBuilder.fromXDR(build({ contract: V040, fn: 'pay_ref', args: refArgs() }), Networks.TESTNET) as Transaction;
    const result = await svc.settle(tx, receipt.payer, 'autonomous-ai-micropayments');
    expect(result.txHash).toBe(fixtures.pay_ref.txHash);
    expect(result.receipt.reader).toBe(receipt.payer);
  });
});

describe('Contratos anteriores (SPLITTER_LEGACY_CONTRACT_IDS)', () => {
  const V040 = 'CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA';
  const withLegacy = (client: SplitterRpc, legacy?: string[]) =>
    new SplitterService({ contractId: V040, legacyContractIds: legacy, publisher: PUBLISHER, rpcUrl: 'http://rpc.invalid', networkPassphrase: Networks.TESTNET, rpc: client });

  it('contractIds lista el activo primero y los anteriores sin repetir', () => {
    expect(withLegacy(fakeRpc(), [CONTRACT, V040]).contractIds()).toEqual([V040, CONTRACT]);
    expect(withLegacy(fakeRpc()).contractIds()).toEqual([V040]);
    expect(new SplitterService({ contractId: '', publisher: '', rpcUrl: 'http://rpc.invalid', networkPassphrase: Networks.TESTNET }).contractIds()).toEqual([]);
  });

  it('un regalo hecho con el contrato anterior se sigue encontrando en la red', async () => {
    const client = fakeRpc({ getTransaction: vi.fn().mockResolvedValue(confirmed(fixtures.gift)) });
    expect(await withLegacy(client).findGift(fixtures.gift.txHash)).toBeNull();
    expect((await withLegacy(client, [CONTRACT]).findGift(fixtures.gift.txHash))?.receipt.reader).toBe(FIXTURE_STUDENT);
  });

  it('los pagos nuevos solo se aceptan con el contrato activo', () => {
    const svc = withLegacy(fakeRpc(), [CONTRACT]);
    expect(() => svc.validateSignedPayment(build(), reader.publicKey(), PAPER)).toThrow(/no invoca el contrato/);
    expect(() => svc.validateSignedPayment(build({ contract: V040 }), reader.publicKey(), PAPER)).not.toThrow();
  });
});
