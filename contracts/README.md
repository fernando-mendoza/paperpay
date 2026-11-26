# Contratos Soroban

## `splitter`: reparto 98% editorial / 2% PaperPay

Cobra lecturas y las reparte en la misma transacción. El USDC va directo de la cuenta que paga a la editorial (monto − comisión) y a la tesorería de PaperPay (comisión). El contrato nunca guarda fondos.

Desde la v0.3.0 también regala listas de lectura (`gift_batch`), cobra propinas (`tip`) y registra devoluciones (`refund`). Las propinas y las devoluciones emiten su propio evento, nunca `paid`, así que no dan acceso a ningún artículo.

Historial de versiones, con su código y sus transacciones: [`CHANGELOG.md`](CHANGELOG.md).

La v0.4.0 suma dos formas de pagar una lectura: `pay_authors` (la parte de la editorial se reparte entre coautores) y `pay_ref` (quien recomendó la lectura se lleva la mitad de la comisión). Las dos emiten `paid`, así que dan acceso igual que `pay`.

La v0.5.0 suma **accesos con vencimiento**, a precio fijo por día:

- `buy_pass`: un pase de N días a todo el catálogo de una editorial;
- `subscribe` y `renew_subscription`: el mismo pase, renovado solo con una autorización (allowance) del token que quien lee firma una vez;
- `sponsor_open`: alguien paga para que un artículo quede abierto para todos N días.

El contrato guarda fechas (hasta cuándo vale cada pase o artículo abierto) y los términos de cada suscripción, pero **sigue sin guardar fondos**: cada cobro va directo de quien paga a la editorial y a la tesorería, también la renovación.

| Función | Qué hace |
| --- | --- |
| `__constructor(token, treasury, fee_bps, pass_day_price, open_day_price)` | Fija el token (SAC de USDC), la tesorería, la comisión en puntos base (`200` = 2%, máximo `1000`) y, desde la v0.5.0, el precio de un día de pase y de un día de artículo abierto (mayores que 0). No cambia después: no hay admin. |
| `pay(payer, paper_id, publisher, amount) -> Split` | Pide la firma de `payer` sobre la llamada con sus argumentos, mueve `amount − fee` a `publisher` y `fee` a la tesorería, y emite el evento `paid` con `payer` como lector. |
| `pay_batch(payer, items) -> Split` | Varios artículos (`Item { paper_id, publisher, amount }`, hasta 10) con **una sola firma**. Reparte cada uno igual que `pay`, agrupa las transferencias por editorial y emite un `paid` por artículo. Es todo o nada: lista vacía, artículo repetido o inválido, o falta de saldo → no se cobra ninguno. Devuelve la suma. |
| `gift(payer, reader, paper_id, publisher, amount) -> Split` | `payer` paga y el acceso queda a nombre de `reader` (un profesor que paga la lectura de un estudiante, por ejemplo). Mismo reparto que `pay`. Solo firma `payer`. |
| `gift_batch(payer, reader, items) -> Split` | `pay_batch` como regalo: `payer` paga hasta 10 artículos y el acceso de todos queda a nombre de `reader` (la lista de lectura de un curso). Mismas reglas: sin repetidos, todo o nada, un `paid` por artículo con `reader`. Solo firma `payer`. *(v0.3.0)* |
| `tip(payer, paper_id, publisher, amount) -> Split` | Propina a la editorial por un artículo, con el mismo reparto que una lectura. Emite `tipped`, **no** `paid`: no da acceso. Solo firma `payer`. *(v0.3.0)* |
| `refund(publisher, reader, paper_id, amount)` | La editorial le devuelve `amount` a `reader` y queda el evento `refunded`. Solo firma la editorial. El contrato no guarda pagos, así que no comprueba el pago original; la comisión ya cobrada no vuelve, porque la tesorería no firma. *(v0.3.0)* |
| `pay_authors(payer, paper_id, authors, amount) -> Split` | Una lectura con coautores. `authors` es una lista de `Share { account, bps }` (de 1 a 10 autores distintos, cada parte en puntos base de lo que le toca a la editorial; entre todas suman `10000`). La tesorería cobra su comisión igual que en `pay`; cada autor recibe su parte redondeada hacia abajo y el residuo va al **primer autor** (no se pierde ningún stroop). Emite un `paid` con el primer autor como `publisher`. Solo firma `payer`. *(v0.4.0)* |
| `pay_ref(payer, paper_id, publisher, amount, referrer) -> RefSplit` | Una lectura recomendada. La editorial recibe lo mismo que en `pay`; de la comisión, la mitad (`REFERRAL_SHARE_BPS` = 5000, redondeada hacia abajo) va a `referrer` y el resto a la tesorería. Devuelve `RefSplit { publisher, fee, referral }`, donde `fee` es lo que recibe la tesorería. Emite `paid` (con la comisión total en `fee`) y `referred`. `referrer` no puede ser `payer`. Solo firma `payer`. *(v0.4.0)* |
| `buy_pass(payer, reader, publisher, days, amount) -> u64` | Un pase de `days` días (1 a 31) para leer todo el catálogo de `publisher`. Cobra exactamente `days × pass_day_price` (`amount` tiene que ser ese monto, para que quien firma lo vea; si no, `WrongPrice`) y lo reparte 98/2 como una lectura. Los días se suman al pase vigente de `reader`; si ya venció, cuentan desde ahora. Un pase no vence más allá de 90 días desde ahora (`TooLong`). `reader` puede ser otra cuenta (pase de regalo). Solo firma `payer`. Emite `pass` y devuelve el vencimiento (segundos Unix, hora del ledger). *(v0.5.0)* |
| `pass_until(reader, publisher) -> u64` | Hasta cuándo vale el pase; 0 si nunca tuvo. Solo lee. *(v0.5.0)* |
| `subscribe(reader, publisher, days, renewals, amount) -> u64` | Suscripción al catálogo de `publisher`. Cobra ya el primer periodo (como `buy_pass`, para `reader`) y, con la misma firma, autoriza al contrato a cobrar `renewals` renovaciones más (1 a 12) del mismo precio: `approve` del token por `amount × renewals`, con vencimiento un día después de lo que cubre la suscripción completa (máximo 150 días: `TooLong`). El dinero de las renovaciones se queda en la wallet de `reader` hasta cada cobro. Una suscripción por lector: una nueva reemplaza a la anterior y a su autorización. Emite `pass` y `subscribed`. *(v0.5.0)* |
| `renew_subscription(reader) -> u64` | Cobra una renovación con la autorización (`transfer_from`), la reparte 98/2 y suma `days` días al pase. **No pide firmas**: la puede enviar cualquiera (la editorial, PaperPay o quien lee), porque solo cobra el precio pactado y solo cuando al pase le falta un día o menos (`RenewTooEarly`); así nunca se cobra más de un periodo por adelantado. Con la última renovación, la suscripción termina. Emite `pass` y `renewed`. *(v0.5.0)* |
| `cancel_subscription(reader)` | Borra las renovaciones pendientes y deja en 0 la autorización del token. El pase ya pagado sigue hasta su vencimiento. Firma `reader`. Emite `unsubscribed`. *(v0.5.0)* |
| `subscription(reader) -> Option<Subscription>` | La suscripción vigente: `{ publisher, days, amount, renewals_left, allowance_until }`. Solo lee. *(v0.5.0)* |
| `sponsor_open(sponsor, paper_id, publisher, days, amount) -> u64` | Un laboratorio, una fundación o la misma editorial paga para que `paper_id` quede **abierto para todos** `days` días (1 a 31). Cobra exactamente `days × open_day_price` y lo reparte 98/2. Los días se suman a la apertura vigente, con el mismo tope de 90 días. Solo firma `sponsor`. Emite `opened`, no `paid`: no es la lectura de una cuenta. Devuelve hasta cuándo queda abierto. *(v0.5.0)* |
| `open_until(publisher, paper_id) -> u64` | Hasta cuándo está abierto para todos el artículo; 0 si nunca se abrió. Solo lee. *(v0.5.0)* |
| `split(amount) -> Split` | El reparto que aplicaría `pay`, sin mover fondos. |
| `config() -> Config` | La configuración fijada al desplegar. |

- **Comisión:** `fee = amount × fee_bps / 10 000`, redondeada hacia abajo. La fracción de stroop queda para la editorial: de $0.50 (`5 000 000`) salen `4 900 000` para la editorial y `100 000` para la tesorería.
- **Evento `paid`:**
  - topics: `["paid", publisher, reader]`;
  - datos: `{ paper_id, payer, amount, fee }`.
  
  Es el comprobante en la red de qué artículo se pagó y quién lo puede leer. En `pay_batch` y `gift_batch` la comisión se redondea por artículo, así que un paquete cobra lo mismo que los `pay` separados.
- **Evento `tipped`** (v0.3.0): topics `["tipped", publisher, payer]`; datos `{ paper_id, amount, fee }`.
- **Evento `refunded`** (v0.3.0): topics `["refunded", publisher, reader]`; datos `{ paper_id, amount }`.
- **Evento `referred`** (v0.4.0): topics `["referred", referrer, payer]`; datos `{ paper_id, publisher, amount, reward }`. Acompaña al `paid` de la misma lectura: `amount` es el precio y `reward` lo que recibió `referrer`.
- **Evento `pass`** (v0.5.0): topics `["pass", publisher, reader]`; datos `{ payer, days, amount, fee, valid_until }`. Lo emiten `buy_pass`, `subscribe` y `renew_subscription`: `reader` puede leer el catálogo de `publisher` hasta `valid_until`.
- **Eventos de la suscripción** (v0.5.0):
  - `subscribed`: topics `["subscribed", publisher, reader]`; datos `{ days, amount, renewals, allowance_until }`;
  - `renewed`: topics `["renewed", publisher, reader]`; datos `{ days, amount, fee, valid_until, renewals_left }`;
  - `unsubscribed`: topics `["unsubscribed", publisher, reader]`; datos `{ renewals_left }`.
- **Evento `opened`** (v0.5.0): topics `["opened", publisher, sponsor]`; datos `{ paper_id, days, amount, fee, open_until }`.
- **Qué guarda la v0.5.0:** en almacenamiento `persistent`, el vencimiento de cada pase (`reader`, `publisher`) y de cada artículo abierto (`publisher`, `paper_id`), y la suscripción de cada lector. Cada entrada se extiende hasta su vencimiento y una semana más. Por eso los topes: 31 días por compra, 90 de acceso acumulado y 150 por suscripción caben en el TTL máximo de testnet (~180 días).
- **La firma de `subscribe` no depende del ledger exacto:** el vencimiento de la autorización es un argumento de `approve` que quien lee firma. Si dependiera del ledger del momento, la red avanzaría entre la simulación y el envío y rechazaría la firma (pasó en la red local; está en [`docs/QA.md`](../docs/QA.md)). Por eso se redondea al día de ledgers (17 280). Solo cambia si entre simular y enviar la red cruza ese múltiplo, una vez al día; entonces basta reintentar.
- **`pay_authors` en la red:** el `paid` lleva solo al primer autor; lo que recibió cada coautor se ve en las transferencias del token de esa transacción.
- **Acceso:** la API solo da acceso por eventos `paid` (`paidReceipts` en `apps/api/src/services/splitter.service.ts`). Una propina o una devolución nunca abren un artículo. Los pases y los artículos abiertos (v0.5.0) existen en testnet, pero la API todavía no los usa.
- **Quién valida qué:** el contrato garantiza el reparto. La API sigue decidiendo el precio y la editorial de cada artículo, y valida que el evento coincida antes de dar acceso.
- **Errores:** `FeeTooHigh` (1), `InvalidAmount` (2), `InvalidPaperId` (3; vacío o de más de 64 bytes), `AmountOverflow` (4), `EmptyBatch` (5), `BatchTooLarge` (6), `DuplicatePaper` (7), `InvalidShares` (8; partes vacías, en 0, de más del 100%, que no suman 100% o con un autor repetido), `TooManyAuthors` (9; más de 10 coautores), `SelfReferral` (10; `referrer` igual a `payer`), `InvalidDays` (11; 0 o más de 31 días), `WrongPrice` (12; `amount` distinto de días × precio por día), `TooLong` (13; un acceso que vencería a más de 90 días o una suscripción de más de 150), `InvalidRenewals` (14; 0 o más de 12), `NoSubscription` (15) y `RenewTooEarly` (16; al pase le falta más de un día).

### Probar

Requiere Rust estable y el target `wasm32v1-none` (`rustup target add wasm32v1-none`).

```bash
pnpm test:contracts            # cargo test: 68 pruebas con un SAC de USDC simulado
```

### Compilar el WASM

`soroban-sdk` 29 solo compila el WASM a través de `stellar-cli` v25.2.0 o posterior (no con `cargo build`):

```bash
cd contracts && stellar contract build
```

### Usarlo desde la terminal

Con `stellar-cli` (v30) se puede desplegar un contrato propio y llamar cada función. Son los mismos comandos con los que se probaron la v0.3.0, la v0.4.0 y la v0.5.0 en testnet.

**1. Cuentas de prueba con USDC.** Quien cobra (editorial, coautor o referido) necesita la trustline de USDC. Quien paga necesita, además, USDC: en testnet se compra con XLM en el DEX.

```bash
USDC=USDC:GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5
stellar keys generate lector --network testnet --fund      # Friendbot le da XLM
LECTOR=$(stellar keys address lector)
stellar tx new change-trust --line "$USDC" --source lector --network testnet
stellar tx new path-payment-strict-receive --send-asset native --send-max 5000000000 \
  --destination "$LECTOR" --dest-asset "$USDC" --dest-amount 20000000 --source lector --network testnet   # 2 USDC
```

**2. Desplegar** (opcional: también se pueden usar los contratos de la tabla de abajo).

```bash
cd contracts && stellar contract build
WASM=target/wasm32v1-none/release/paperpay_splitter.wasm
stellar contract upload --wasm "$WASM" --source lector --network testnet
stellar contract deploy --wasm-hash "$(sha256sum "$WASM" | cut -c1-64)" --source lector --network testnet \
  -- --token CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA \
     --treasury GAB2NXBPZMJEPZPXMAAEZDJA7SB6ZQRHTT2KF4Z7LMOEAJKSP4JBCWRV --fee_bps 200 \
     --pass_day_price 10000000 --open_day_price 50000000   # desde la v0.5.0: 1 USDC por día de pase, 5 por día abierto
```

**3. Llamar cada función.** Los montos van en stroops (7 decimales: `5000000` = 0.50 USDC). Dentro de un JSON, los `i128` van como texto (`"5000000"`) y los `u32` como número. `--source` es la cuenta que firma: quien paga en todas, salvo en `refund`, donde firma la editorial.

```bash
ID=CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA   # v0.4.0, o el contrato que desplegaste
inv() { stellar contract invoke --id "$ID" --network testnet "$@"; }
P=crispr-nanomedicine-biotech
ED=GA3MOPOJ374G4APKR4RE7VPXF5CEQKER3FA3LZ2XSAARIRSZEF6QXUZI      # editorial demo (tiene trustline)

inv --source lector -- pay        --payer "$LECTOR" --paper_id "$P" --publisher "$ED" --amount 5000000
inv --source lector -- pay_batch  --payer "$LECTOR" --items "[{\"paper_id\":\"$P\",\"publisher\":\"$ED\",\"amount\":\"5000000\"}]"
inv --source lector -- gift       --payer "$LECTOR" --reader "$OTRA" --paper_id "$P" --publisher "$ED" --amount 5000000
inv --source lector -- gift_batch --payer "$LECTOR" --reader "$OTRA" --items "[{\"paper_id\":\"$P\",\"publisher\":\"$ED\",\"amount\":\"5000000\"}]"
inv --source lector -- tip        --payer "$LECTOR" --paper_id "$P" --publisher "$ED" --amount 5000000
inv --source lector -- pay_authors --payer "$LECTOR" --paper_id "$P" --amount 5000000 \
  --authors "[{\"account\":\"$ED\",\"bps\":7000},{\"account\":\"$COAUTOR\",\"bps\":3000}]"
inv --source lector -- pay_ref    --payer "$LECTOR" --paper_id "$P" --publisher "$ED" --amount 5000000 --referrer "$REFERIDO"
inv --source editorial -- refund  --publisher "$EDITORIAL" --reader "$LECTOR" --paper_id "$P" --amount 4900000
inv --source lector --send no -- config                       # solo lee; no envía nada

# v0.5.0: accesos con vencimiento (precios de la configuración: 1 USDC por día de pase, 5 por día abierto)
inv --source lector -- buy_pass  --payer "$LECTOR" --reader "$LECTOR" --publisher "$ED" --days 1 --amount 10000000
inv --source lector -- subscribe --reader "$LECTOR" --publisher "$ED" --days 1 --renewals 2 --amount 10000000
inv --source otra   -- renew_subscription --reader "$LECTOR"   # cualquier cuenta la envía; el lector no firma
inv --source lector -- cancel_subscription --reader "$LECTOR"
inv --source lector -- sponsor_open --sponsor "$LECTOR" --paper_id "$P" --publisher "$ED" --days 1 --amount 50000000
inv --source lector --send no -- pass_until --reader "$LECTOR" --publisher "$ED"
inv --source lector --send no -- open_until --publisher "$ED" --paper_id "$P"
```

`$OTRA` es cualquier cuenta: recibir un regalo no pide firma ni trustline, y la cuenta ni siquiera tiene que existir en la red. `$COAUTOR` y `$REFERIDO` sí necesitan la trustline de USDC (paso 1), porque reciben dinero. `refund` necesita una editorial con llave propia (`$EDITORIAL`, cuenta `editorial`) y saldo en USDC. `renew_subscription` la puede enviar cualquier cuenta con XLM para la comisión de red (`otra`), sin USDC.

### Despliegue en testnet

Datos completos en [`deployments/testnet.json`](deployments/testnet.json).

> **Qué versión usa el sitio.** El sitio y la API usan la **v0.4.0** (`productionContractId`) desde el 25 de noviembre de 2026 (CDMX; 00:18 UTC del 26), con `pay`, `pay_ref` (referidos) y `gift`. La v0.4.0 tiene todo lo de la v0.3.0 (`gift_batch`, `tip` y `refund`) más `pay_authors` y `pay_ref`. La v0.2.0, que usó el sitio del 23 al 25 de noviembre, quedó en `SPLITTER_LEGACY_CONTRACT_IDS`: sus regalos se pueden reclamar y sus lecturas siguen en el contador. Los pagos nuevos van solo a la v0.4.0. La **v0.5.0** (pases, suscripción y artículos abiertos) está desplegada y probada en testnet desde el 25 de noviembre de 2026 (CDMX; 05:04 UTC del 26), pero el sitio y la API todavía no la usan.

| v0.5.0 | Valor |
| --- | --- |
| Contrato | [`CA2QHBXMF6KIBBFT5ATQYMFDP75246RQV6NC6YA55QJZX3CRVFUE4RT2`](https://stellar.expert/explorer/testnet/contract/CA2QHBXMF6KIBBFT5ATQYMFDP75246RQV6NC6YA55QJZX3CRVFUE4RT2) |
| WASM (sha256) | `913ac9b4b43766bcc60e685d6a985e82dad677dc17d2c7f6a6aa892ca7b3bd17` (28,642 bytes), igual al que se baja de la red |
| Constructor | El de v0.2.0 más `pass_day_price` = 10000000 (1 USDC por día de pase) y `open_day_price` = 50000000 (5 USDC por día abierto) |
| `buy_pass` | [`747d2015…`](https://stellar.expert/explorer/testnet/tx/747d20154da627c210dde94b99a99b45042c98fb8985aa34cbbc69d4257b57a8): un lector le regala a otra cuenta un pase de 1 día al catálogo de la editorial demo (1.00 USDC). 0.98 a la editorial demo, 0.02 a la tesorería y un evento `pass` con esa cuenta como `reader` |
| `subscribe` | [`e4b2b08d…`](https://stellar.expert/explorer/testnet/tx/e4b2b08dc02e763d27be68bef87e3947d4446fc3cd8f903585bc320fe5f6b549): suscripción de 1 día por periodo con 2 renovaciones. Cobra el primer día (0.98 / 0.02) y, con la misma firma, autoriza al contrato por 2.00 USDC. Eventos `pass` y `subscribed` |
| `renew_subscription` | [`3f586bef…`](https://stellar.expert/explorer/testnet/tx/3f586befa2582e47633812238990ab691facb51e22c5c47b21fb8395a18aeff5): la envía **otra cuenta, sin USDC**; el lector no firma. 0.98 / 0.02 desde la wallet del lector, el pase llega a 2 días y queda 1 renovación (autorización en 1.00 USDC). Eventos `pass` y `renewed` |
| `sponsor_open` | [`a49b8687…`](https://stellar.expert/explorer/testnet/tx/a49b86873d7cc33103787f9b7af5cb474469efc2494f6a4594cc437f87b313fc): un patrocinador abre `crispr-nanomedicine-biotech` para todos 1 día (5.00 USDC). 4.90 a la editorial demo, 0.10 a la tesorería y un evento `opened` |

Después de las cuatro, el contrato tiene saldo 0, y una renovación adelantada falla con `RenewTooEarly` (#16). Las cuentas son desechables (Friendbot) y ninguna tiene poder sobre el contrato.

| v0.4.0 | Valor |
| --- | --- |
| Contrato | [`CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA`](https://stellar.expert/explorer/testnet/contract/CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA) |
| WASM (sha256) | `b97fffc81b053f597aaff0cb39b69a21005835ba412b8d8cf51681db1a6135aa` (15,339 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.2.0 |
| `pay_authors` | [`23a68ac1…`](https://stellar.expert/explorer/testnet/tx/23a68ac1be070324569dcb1a39b0e9ff1a80283a4809e63112ca0a3e648a4867): 0.50 USDC por un artículo con 3 coautores (50/30/20). 0.245 a la editorial demo, 0.147 y 0.098 a los otros dos, 0.01 a la tesorería y un evento `paid` a nombre del primer autor |
| `pay_ref` | [`ab2ca691…`](https://stellar.expert/explorer/testnet/tx/ab2ca6914b583415bf821e3223b651a4ab2d73863c827eb0634fb48752f7aa22): 0.50 USDC recomendado por otra cuenta. 0.49 a la editorial demo, 0.005 a quien recomendó, 0.005 a la tesorería y los eventos `paid` y `referred` |
| `pay_ref` desde el sitio | [`d3a130f6…`](https://stellar.expert/explorer/testnet/tx/d3a130f6aff66928b9d4c54288901d0507d61b5995918753bfe83086754d1197): el primero por `paperpay.blockchains.tf` (Pages Function y API de la VPS). Mismo reparto, 0.49 / 0.005 / 0.005 |

| v0.3.0 | Valor |
| --- | --- |
| Contrato | [`CCQRUTXBSWOSOHIZIEJ637HIMJZ6CZN5ZYSMTPGBTSMFAEXY6O5QHEQD`](https://stellar.expert/explorer/testnet/contract/CCQRUTXBSWOSOHIZIEJ637HIMJZ6CZN5ZYSMTPGBTSMFAEXY6O5QHEQD) |
| WASM (sha256) | `2af32e3a2017dedc6105c13e161a4a04bdad328c0ee271edbb2bad1d712318fb` (10,635 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.2.0 |
| `gift_batch` | [`70d30919…`](https://stellar.expert/explorer/testnet/tx/70d309192251ee041afadbbd51cd938c0fcab28fec3b258560241204544d07f9): 2 artículos (1.00 USDC) para una cuenta que no existe en la red. 0.98 a la editorial demo en una sola transferencia, 0.02 a la tesorería y 2 eventos `paid` con esa cuenta como `reader` |
| `tip` | [`36e14727…`](https://stellar.expert/explorer/testnet/tx/36e14727c832a99a448d80cece792899aa117599a5b301eadde96722434ed5d5): propina de 0.50 USDC a la editorial demo. 0.49 / 0.01 y un evento `tipped`, ningún `paid` |
| `refund` | [`3bc22905…`](https://stellar.expert/explorer/testnet/tx/3bc229057ea0312ea574dfee50cde01280a7fb647fc58cc5a8d2d37e1c0910db): una editorial desechable devuelve 0.49 USDC del [pago `a10772f8…`](https://stellar.expert/explorer/testnet/tx/a10772f8faa8ab883fba19824caadc69412378b92396d6b6028c30745b9dea3e). Evento `refunded`; la comisión de 0.01 se queda en la tesorería |

Las cuentas de esta prueba son desechables (Friendbot): sus llaves se borraron al terminar y ninguna tiene poder sobre el contrato.

**v0.2.0 (en producción del 23 al 25 de noviembre; hoy, contrato anterior):**

| Dato | Valor |
| --- | --- |
| Contrato (v0.2.0) | [`CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5`](https://stellar.expert/explorer/testnet/contract/CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5) |
| Constructor | token = SAC de USDC testnet (`CBIELTK6…`), treasury = tesorería de PaperPay (`GAB2…CWRV`), `fee_bps` = 200 |
| WASM (sha256) | `4868dd0233d3153fcd128c5e4f8ba97c7d0c544fc49a6a1174bdb68cfcf2cfa3`, igual al que se baja de la red con `stellar contract fetch` |
| Editorial demo | `GA3MOPOJ374G4APKR4RE7VPXF5CEQKER3FA3LZ2XSAARIRSZEF6QXUZI`: una sola cuenta recibe el 98% de las editoriales ficticias del catálogo |
| `pay_batch` | [`551ae39b…`](https://stellar.expert/explorer/testnet/tx/551ae39b2fd42f247cbbfc355e5dfffbb0d2fbbec2514e5a9756b92ef4b21dff): los 6 artículos de `/top` con una firma (3.00 USDC). Resultado: 2.94 a la editorial y 0.06 a la tesorería en 2 transferencias, con 6 eventos `paid` |
| `gift` | [`8dac0f7e…`](https://stellar.expert/explorer/testnet/tx/8dac0f7e574f6980da0baf49e2db3cc53191e84457e5fb38018d732dcd092753): 0.50 USDC por `stablecoins-emerging-markets` para otra cuenta, que ni siquiera existe en la red. El evento `paid` lleva a esa cuenta como `reader` |

La v0.1.0 ([`CDOUF6…BPOZ`](https://stellar.expert/explorer/testnet/contract/CDOUF6BBLKAXPT6EJAF2SOAUMKX3RASZFNHD7ORMIQZUTH4G6V66BPOZ), solo `pay`, con el [pago de prueba `cdda69b1…`](https://stellar.expert/explorer/testnet/tx/cdda69b15feb5767ddb3b8b436386a953c869e2ed30476a72028234ad130c489)) sigue en la red, pero queda reemplazada. Ninguna versión tiene admin: la cuenta que despliega es desechable y no conserva ningún poder sobre el contrato.

### Estado

Desplegado en testnet y **activo** desde el 23 de noviembre de 2026 en la API de la VPS y en [paperpay.blockchains.tf](https://paperpay.blockchains.tf/): con Freighter, cada lectura paga por `pay`. Prueba de punta a punta contra el sitio publicado: [`96dd614e…`](https://stellar.expert/explorer/testnet/tx/96dd614e5a3b13c2e0d7dfc58d1e5b6457f554dc22754e25ce89a719293e74d9), 0.49 a la editorial y 0.01 a la tesorería. Desde el 24 de noviembre, el sitio también ofrece `gift` ("Regálalo"): quien regala paga y comparte un enlace con el que otra persona lee sin wallet (ver [la API](../apps/api/README.md#regalo-por-enlace-gift)); prueba contra el sitio publicado: [`3fe887ee…`](https://stellar.expert/explorer/testnet/tx/3fe887ee932f4e260267dc80b4a2a7c5d31ecf1d6b776a7eea7bc6931d517f7e). Pollar y el sitio de Vercel siguen con transferencias completas a la tesorería. `pay_batch` funciona en la red, pero el sitio todavía no lo ofrece.
