# Contratos Soroban

## `splitter`: reparto 98% editorial / 2% PaperPay

Cobra lecturas y las reparte en la misma transacción. El USDC va directo de la cuenta que paga a la editorial (monto − comisión) y a la tesorería de PaperPay (comisión). El contrato nunca guarda fondos.

Desde la v0.3.0 también regala listas de lectura (`gift_batch`), cobra propinas (`tip`) y registra devoluciones (`refund`). Las propinas y las devoluciones emiten su propio evento, nunca `paid`, así que no dan acceso a ningún artículo.

La v0.4.0 suma dos formas de pagar una lectura: `pay_authors` (la parte de la editorial se reparte entre coautores) y `pay_ref` (quien recomendó la lectura se lleva la mitad de la comisión). Las dos emiten `paid`, así que dan acceso igual que `pay`.

| Función | Qué hace |
| --- | --- |
| `__constructor(token, treasury, fee_bps)` | Fija el token (SAC de USDC), la tesorería y la comisión en puntos base (`200` = 2%, máximo `1000`). No cambia después: no hay admin. |
| `pay(payer, paper_id, publisher, amount) -> Split` | Pide la firma de `payer` sobre la llamada con sus argumentos, mueve `amount − fee` a `publisher` y `fee` a la tesorería, y emite el evento `paid` con `payer` como lector. |
| `pay_batch(payer, items) -> Split` | Varios artículos (`Item { paper_id, publisher, amount }`, hasta 10) con **una sola firma**. Reparte cada uno igual que `pay`, agrupa las transferencias por editorial y emite un `paid` por artículo. Es todo o nada: lista vacía, artículo repetido o inválido, o falta de saldo → no se cobra ninguno. Devuelve la suma. |
| `gift(payer, reader, paper_id, publisher, amount) -> Split` | `payer` paga y el acceso queda a nombre de `reader` (un profesor que paga la lectura de un estudiante, por ejemplo). Mismo reparto que `pay`. Solo firma `payer`. |
| `gift_batch(payer, reader, items) -> Split` | `pay_batch` como regalo: `payer` paga hasta 10 artículos y el acceso de todos queda a nombre de `reader` (la lista de lectura de un curso). Mismas reglas: sin repetidos, todo o nada, un `paid` por artículo con `reader`. Solo firma `payer`. *(v0.3.0)* |
| `tip(payer, paper_id, publisher, amount) -> Split` | Propina a la editorial por un artículo, con el mismo reparto que una lectura. Emite `tipped`, **no** `paid`: no da acceso. Solo firma `payer`. *(v0.3.0)* |
| `refund(publisher, reader, paper_id, amount)` | La editorial le devuelve `amount` a `reader` y queda el evento `refunded`. Solo firma la editorial. El contrato no guarda pagos, así que no comprueba el pago original; la comisión ya cobrada no vuelve, porque la tesorería no firma. *(v0.3.0)* |
| `pay_authors(payer, paper_id, authors, amount) -> Split` | Una lectura con coautores. `authors` es una lista de `Share { account, bps }` (de 1 a 10 autores distintos, cada parte en puntos base de lo que le toca a la editorial; entre todas suman `10000`). La tesorería cobra su comisión igual que en `pay`; cada autor recibe su parte redondeada hacia abajo y el residuo va al **primer autor** (no se pierde ningún stroop). Emite un `paid` con el primer autor como `publisher`. Solo firma `payer`. *(v0.4.0)* |
| `pay_ref(payer, paper_id, publisher, amount, referrer) -> RefSplit` | Una lectura recomendada. La editorial recibe lo mismo que en `pay`; de la comisión, la mitad (`REFERRAL_SHARE_BPS` = 5000, redondeada hacia abajo) va a `referrer` y el resto a la tesorería. Devuelve `RefSplit { publisher, fee, referral }`, donde `fee` es lo que recibe la tesorería. Emite `paid` (con la comisión total en `fee`) y `referred`. `referrer` no puede ser `payer`. Solo firma `payer`. *(v0.4.0)* |
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
- **`pay_authors` en la red:** el `paid` lleva solo al primer autor; lo que recibió cada coautor se ve en las transferencias del token de esa transacción.
- **Acceso:** la API solo da acceso por eventos `paid` (`paidReceipts` en `apps/api/src/services/splitter.service.ts`). Una propina o una devolución nunca abren un artículo.
- **Quién valida qué:** el contrato garantiza el reparto. La API sigue decidiendo el precio y la editorial de cada artículo, y valida que el evento coincida antes de dar acceso.
- **Errores:** `FeeTooHigh` (1), `InvalidAmount` (2), `InvalidPaperId` (3; vacío o de más de 64 bytes), `AmountOverflow` (4), `EmptyBatch` (5), `BatchTooLarge` (6), `DuplicatePaper` (7), `InvalidShares` (8; partes vacías, en 0, de más del 100%, que no suman 100% o con un autor repetido), `TooManyAuthors` (9; más de 10 coautores), `SelfReferral` (10; `referrer` igual a `payer`).

### Probar

Requiere Rust estable y el target `wasm32v1-none` (`rustup target add wasm32v1-none`).

```bash
pnpm test:contracts            # cargo test: 43 pruebas con un SAC de USDC simulado
```

### Compilar el WASM

`soroban-sdk` 29 solo compila el WASM a través de `stellar-cli` v25.2.0 o posterior (no con `cargo build`):

```bash
cd contracts && stellar contract build
```

### Despliegue en testnet

Datos completos en [`deployments/testnet.json`](deployments/testnet.json).

> **Dos versiones en la red.** El sitio y la API usan **v0.2.0** (`productionContractId`). La **v0.3.0** suma `gift_batch`, `tip` y `refund`, y está desplegada y probada en testnet, pero el sitio todavía no la usa: cambiarla es cambiar `SPLITTER_CONTRACT_ID` en la API (un despliegue a producción).

| v0.3.0 | Valor |
| --- | --- |
| Contrato | [`CCQRUTXBSWOSOHIZIEJ637HIMJZ6CZN5ZYSMTPGBTSMFAEXY6O5QHEQD`](https://stellar.expert/explorer/testnet/contract/CCQRUTXBSWOSOHIZIEJ637HIMJZ6CZN5ZYSMTPGBTSMFAEXY6O5QHEQD) |
| WASM (sha256) | `2af32e3a2017dedc6105c13e161a4a04bdad328c0ee271edbb2bad1d712318fb` (10,635 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.2.0 |
| `gift_batch` | [`70d30919…`](https://stellar.expert/explorer/testnet/tx/70d309192251ee041afadbbd51cd938c0fcab28fec3b258560241204544d07f9): 2 artículos (1.00 USDC) para una cuenta que no existe en la red. 0.98 a la editorial demo en una sola transferencia, 0.02 a la tesorería y 2 eventos `paid` con esa cuenta como `reader` |
| `tip` | [`36e14727…`](https://stellar.expert/explorer/testnet/tx/36e14727c832a99a448d80cece792899aa117599a5b301eadde96722434ed5d5): propina de 0.50 USDC a la editorial demo. 0.49 / 0.01 y un evento `tipped`, ningún `paid` |
| `refund` | [`3bc22905…`](https://stellar.expert/explorer/testnet/tx/3bc229057ea0312ea574dfee50cde01280a7fb647fc58cc5a8d2d37e1c0910db): una editorial desechable devuelve 0.49 USDC del [pago `a10772f8…`](https://stellar.expert/explorer/testnet/tx/a10772f8faa8ab883fba19824caadc69412378b92396d6b6028c30745b9dea3e). Evento `refunded`; la comisión de 0.01 se queda en la tesorería |

Las cuentas de esta prueba son desechables (Friendbot): sus llaves se borraron al terminar y ninguna tiene poder sobre el contrato.

**v0.2.0 (en producción):**

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
